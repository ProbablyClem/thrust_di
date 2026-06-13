use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, Fields, ItemStruct, ItemTrait, Type, TypeParamBound,
};

fn no_op_attr(item: TokenStream) -> TokenStream {
    let allow: TokenStream = "#[allow(dead_code)]".parse().unwrap();
    let mut out = allow;
    out.extend(item);
    out
}

/// Marks a struct as an injectable component. Beyond the historical
/// `#[allow(dead_code)]`, this rewrites any field written as a bare trait object
/// (`dyn Trait`) into `Arc<dyn Trait>`, so dependencies can be declared without
/// the `Arc` boilerplate. Fields already wrapped in `Arc<...>` (or any other
/// type) are left untouched.
#[proc_macro_attribute]
pub fn service(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_struct = parse_macro_input!(item as ItemStruct);

    if let Fields::Named(named) = &mut item_struct.fields {
        for field in named.named.iter_mut() {
            if let Type::TraitObject(_) = &field.ty {
                let obj = &field.ty;
                field.ty = parse_quote!(::std::sync::Arc<#obj>);
            }
        }
    }

    quote! {
        #[allow(dead_code)]
        #item_struct
    }
    .into()
}

/// Marks a trait as an injectable abstraction. Adds `Send + Sync` supertrait
/// bounds (if not already present) so the trait can be stored as
/// `Arc<dyn Trait>` inside the thread-shared container.
#[proc_macro_attribute]
pub fn interface(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_trait = parse_macro_input!(item as ItemTrait);

    let has_bound = |name: &str| {
        item_trait.supertraits.iter().any(|b| match b {
            TypeParamBound::Trait(tb) => tb.path.is_ident(name),
            _ => false,
        })
    };
    let (needs_send, needs_sync) = (!has_bound("Send"), !has_bound("Sync"));
    if needs_send {
        item_trait.supertraits.push(parse_quote!(Send));
    }
    if needs_sync {
        item_trait.supertraits.push(parse_quote!(Sync));
    }

    quote! {
        #[allow(dead_code)]
        #item_trait
    }
    .into()
}

#[proc_macro_attribute]
pub fn get(_attr: TokenStream, item: TokenStream) -> TokenStream {
    no_op_attr(item)
}

#[proc_macro_attribute]
pub fn post(_attr: TokenStream, item: TokenStream) -> TokenStream {
    no_op_attr(item)
}

#[proc_macro_attribute]
pub fn put(_attr: TokenStream, item: TokenStream) -> TokenStream {
    no_op_attr(item)
}

#[proc_macro_attribute]
pub fn delete(_attr: TokenStream, item: TokenStream) -> TokenStream {
    no_op_attr(item)
}

#[proc_macro_attribute]
pub fn patch(_attr: TokenStream, item: TokenStream) -> TokenStream {
    no_op_attr(item)
}

#[proc_macro_attribute]
pub fn bean(_attr: TokenStream, item: TokenStream) -> TokenStream {
    no_op_attr(item)
}

#[proc_macro_attribute]
pub fn layer(_attr: TokenStream, item: TokenStream) -> TokenStream {
    no_op_attr(item)
}

#[proc_macro]
pub fn init(_: TokenStream) -> TokenStream {
    r#"include!(concat!(env!("OUT_DIR"), "/generated.rs"));"#
        .parse()
        .unwrap()
}
