use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, parse_quote, Fields, ItemStruct, Type, TypeParamBound};

fn no_op_attr(item: TokenStream) -> TokenStream {
    let allow: TokenStream = "#[allow(dead_code)]".parse().unwrap();
    let mut out = allow;
    out.extend(item);
    out
}

/// Marks a struct as an injectable component. Beyond the historical
/// `#[allow(dead_code)]`, this rewrites a field written as a bare trait object
/// (`dyn Trait`) into `Arc<dyn Trait + Send + Sync>` — a trait object, always
/// dynamically dispatched. This keeps the field a true abstraction in every
/// build, so it behaves identically in production and tests and can be
/// constructed with a mock (`Arc::new(MockRepo)`) in unit *and* integration
/// tests.
///
/// `Send + Sync` is added at the use-site so the trait definition itself stays
/// bound-free. Fields written as explicit `Arc<dyn Trait>` are already trait
/// objects and left untouched (you supply the bounds); concrete `Arc<T>` and
/// other fields are also left untouched.
#[proc_macro_attribute]
pub fn service(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_struct = parse_macro_input!(item as ItemStruct);

    if let Fields::Named(named) = &mut item_struct.fields {
        for field in named.named.iter_mut() {
            if let Type::TraitObject(obj) = &field.ty {
                if let Some(trait_ident) = first_trait_ident(obj) {
                    field.ty = parse_quote!(::std::sync::Arc<dyn #trait_ident + Send + Sync>);
                }
            }
        }
    }

    quote! {
        #[allow(dead_code)]
        #item_struct
    }
    .into()
}

/// Last path segment of the first trait bound in a trait object, e.g.
/// `dyn UserRepository + Send` -> `UserRepository`.
fn first_trait_ident(obj: &syn::TypeTraitObject) -> Option<proc_macro2::Ident> {
    obj.bounds.iter().find_map(|b| match b {
        TypeParamBound::Trait(tb) => tb.path.segments.last().map(|s| s.ident.clone()),
        _ => None,
    })
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
