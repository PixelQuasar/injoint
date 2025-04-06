extern crate proc_macro;
use crate::utils::snake_to_camel;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, DeriveInput, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, ItemStruct,
    PatType, Signature, Token, Type,
};

mod utils;

#[proc_macro_derive(Broadcastable)]
pub fn derive_broadcastable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = input.ident;

    let expanded = quote! {
        impl injoint::utils::types::Broadcastable for #struct_name {}
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn reducer_struct(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input: ItemStruct = parse_macro_input!(item);

    let reducer_name = input.clone().ident;

    let args: Vec<Ident> =
        parse_macro_input!(attr with Punctuated::<Ident, Token![,]>::parse_terminated)
            .into_iter()
            .collect();

    let state_struct = args[0].clone();

    let expanded = quote! {
        impl injoint::utils::types::Broadcastable for #state_struct {}
        impl injoint::utils::types::Broadcastable for #reducer_name {}
        #[derive(serde::Serialize)]
        #input
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn reducer_actions(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input: ItemImpl = parse_macro_input!(item);

    let implementation = input.clone();

    let args: Vec<Ident> =
        parse_macro_input!(attr with Punctuated::<Ident, Token![,]>::parse_terminated)
            .into_iter()
            .collect();

    let state_struct = args[0].clone();

    let reducer_name = match *input.self_ty {
        Type::Path(ref type_path) => &type_path.path.segments.last().unwrap().ident,
        _ => panic!("Invalid impl"),
    };
    let reducer_span = reducer_name.span();

    let methods = input
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(x) => Some(x.clone()),
            _ => None,
        })
        .collect::<Vec<ImplItemFn>>();

    fn parse_action_name(sig: &Signature) -> proc_macro2::TokenStream {
        let span = sig.ident.clone().span();
        let name = Ident::new(
            &format!("Action{}", snake_to_camel(&sig.ident.to_string())),
            span,
        );
        let expanded = quote! {#name};

        expanded
    }

    fn parse_action_args(sig: &Signature) -> Vec<&PatType> {
        let mut args = sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Typed(item) => Some(item),
                _ => None,
            })
            .collect::<Vec<_>>();

        args.remove(0); // remove "client_id: u64" arg

        args
    }

    fn parse_action_arg_types(sig: &Signature) -> Vec<proc_macro2::TokenStream> {
        let span = sig.ident.clone().span();
        parse_action_args(sig)
            .iter()
            .map(|item| {
                Ident::new(&item.ty.clone().to_token_stream().to_string(), span).to_token_stream()
            })
            .collect::<Vec<_>>()
    }

    fn parse_action_arg_names(sig: &Signature) -> Vec<proc_macro2::TokenStream> {
        let span = sig.ident.clone().span();
        parse_action_args(sig)
            .iter()
            .map(|item| {
                Ident::new(&item.pat.clone().to_token_stream().to_string(), span).to_token_stream()
            })
            .collect::<Vec<_>>()
    }

    let actions = methods
        .clone()
        .iter()
        .map(|&ref method| {
            let name = parse_action_name(&method.sig);
            let args = parse_action_arg_types(&method.sig);

            quote! {
                #name(#(#args),*)
            }
        })
        .collect::<Vec<_>>();

    let action_enum_name = Ident::new(&format!("Action{}", reducer_name), reducer_span);

    let action_names = methods
        .clone()
        .iter()
        .map(|method| {
            let enum_name = &action_enum_name.clone();
            let action_name = parse_action_name(&method.sig);
            // let action_name_str =
            //     Ident::new(&format!("{}", action_name), action_name.span()).to_token_stream();
            let action_name_str = &format!("{}", action_name);
            let args = parse_action_arg_names(&method.sig);

            let result = quote! {
                #enum_name::#action_name(#(#args),*) => String::from(#action_name_str)
            };

            result
        })
        .collect::<Vec<_>>();

    let action_handlers = methods
        .clone()
        .iter()
        .map(|method| {
            let enum_name = &action_enum_name.clone();
            let action_name = parse_action_name(&method.sig);
            let method_name = method.sig.ident.clone();
            let args = parse_action_arg_names(&method.sig);

            let result = quote! {
                #enum_name::#action_name(#(#args),*) => self.#method_name(client_id, #(#args),*).await?
            };

            result
        })
        .collect::<Vec<_>>();

    let enum_name = &action_enum_name.clone();

    let expanded = quote! {
        #implementation

        #[derive(serde::Deserialize, Debug)]
        #[serde(tag = "type", content = "data")]
        enum #enum_name {
            #(#actions),*
        }

        impl injoint::utils::types::Receivable for #enum_name {}

        impl injoint::dispatcher::Dispatchable for #reducer_name {
            type Action = #enum_name;
            type State = #state_struct;

            fn get_state(&self) -> #state_struct {
                self.state.clone()
            }

            async fn dispatch(
                &mut self,
                client_id: u64,
                action:  #enum_name,
            ) -> Result<injoint::dispatcher::ActionResponse<#state_struct>, String> {
                let name = match &action {
                    #(#action_names),*
                };

                let msg = match action {
                    #(#action_handlers),*
                };

                Ok(injoint::dispatcher::ActionResponse {
                    status: name,
                    state: self.state.clone(),
                    author: client_id,
                    data: msg,
                })
            }

            async fn extern_dispatch(
                &mut self,
                client_id: u64,
                action: &str,
            ) -> Result<injoint::dispatcher::ActionResponse<#state_struct>, String> {
                let action: #enum_name = serde_json::from_str(action).unwrap();
                self.dispatch(client_id, action).await
            }

            fn get_state(&self) -> #state_struct {
                self.state.clone()
            }
        }
    };

    TokenStream::from(expanded)
}
