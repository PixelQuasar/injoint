extern crate proc_macro;
use crate::utils::snake_to_camel;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::punctuated::Punctuated;
use syn::{
    parse_macro_input, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, PatType, Signature, Token,
    Type,
};

mod utils;

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
        quote! {#name}
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

            let expanded = quote! {
                #name(#(#args),*)
            };

            expanded
        })
        .collect::<Vec<_>>();

    let action_enum_name = Ident::new(&format!("Action{}", reducer_name), reducer_span);

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

        #[derive(Deserialize, Debug)]
        #[serde(tag = "type", content = "data")]
        enum #enum_name {
            #(#actions),*
        }

        impl Receivable for #enum_name {}

        impl Dispatchable for #reducer_name {
            type Action = #enum_name;
            type Response = #state_struct;

            async fn dispatch(
                &mut self,
                client_id: u64,
                action:  #enum_name,
            ) -> Result<ActionResponse<#state_struct>, String> {
                let msg = match action {
                    #(#action_handlers),*
                };

                Ok(ActionResponse {
                    state: self.state.clone(),
                    author: client_id,
                    data: msg,
                })
            }
        }
    };

    TokenStream::from(expanded)
}
