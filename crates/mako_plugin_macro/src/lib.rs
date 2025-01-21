#![feature(box_patterns)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::{parse2, parse_quote, DeriveInput, FnArg, ImplItem, ImplItemFn, ItemImpl, Pat, PatType};

#[proc_macro_derive(mako_plugin)]
pub fn mako_plugin(input: TokenStream) -> TokenStream {
    handler_create(input.into()).into()
}

fn handler_create(input: TokenStream2) -> TokenStream2 {
    let ast = parse2::<DeriveInput>(input).unwrap();
    let struct_name = &ast.ident;
    let ts = quote! {
      #[no_mangle]
      pub fn _plugin_create(option: serde_json::Value) -> std::sync::Arc<dyn Plugin> {
        std::sync::Arc::new(#struct_name::new(option))
      }
    };
    ts
}

#[proc_macro_attribute]
pub fn with_local(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_local(_attr.into(), item.into()).into()
}

fn handle_local(_attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let ast = parse2::<ItemImpl>(item).unwrap();
    let items = &ast.items;
    let mut expand_items: Vec<ImplItem> = vec![];

    items.iter().for_each(|impl_item| {
        if let ImplItem::Fn(f) = impl_item {
            let body = &f.block;

            let has_context = f.sig.inputs.iter().any(|f| {
                if let FnArg::Typed(PatType {
                    pat: box Pat::Ident(i),
                    ..
                }) = f
                {
                    i.ident.to_string().contains("context")
                } else {
                    false
                }
            });
            let block = if has_context {
                parse_quote!({
                    mako::swc_core::common::GLOBALS.set(&context.meta.script.globals, ||
                        #body
                    )
                })
            } else {
                parse_quote!(
                    #body
                )
            };

            expand_items.push(ImplItem::Fn(ImplItemFn { block, ..f.clone() }));
        }
    });
    let a = ItemImpl {
        items: expand_items,
        ..ast
    };
    a.into_token_stream()
}
