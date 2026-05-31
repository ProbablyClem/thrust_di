use proc_macro::TokenStream;

fn no_op_attr(item: TokenStream) -> TokenStream {
    let allow: TokenStream = "#[allow(dead_code)]".parse().unwrap();
    let mut out = allow;
    out.extend(item);
    out
}

#[proc_macro_attribute]
pub fn service(_attr: TokenStream, item: TokenStream) -> TokenStream {
    no_op_attr(item)
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

#[proc_macro]
pub fn init(_: TokenStream) -> TokenStream {
    r#"include!(concat!(env!("OUT_DIR"), "/generated.rs"));"#
        .parse()
        .unwrap()
}
