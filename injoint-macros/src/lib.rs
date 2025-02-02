extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Ident, ItemStruct, Token};

#[proc_macro_attribute]
pub fn build_injoint(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input: ItemStruct = parse_macro_input!(item);

    let args: Vec<Ident> =
        parse_macro_input!(attr with Punctuated::<Ident, Token![,]>::parse_terminated)
            .into_iter()
            .collect();

    let mut strs: Vec<String> = Vec::new();

    for arg in args.iter() {
        strs.push(arg.to_string());
    }

    let struct_name = &input.ident;
    let fields = &input.fields;

    let reducers: Vec<_> = strs
        .iter()
        .map(|method| {
            let ident = Ident::new(method, input.ident.span());
            quote! {
                #ident: i32,
            }
        })
        .collect();

    let expanded = quote! {
        struct #struct_name {
            #fields
            #(#reducers)*
        }
    };

    TokenStream::from(expanded)
}
