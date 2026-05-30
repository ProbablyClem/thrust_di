use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn service(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let allow: TokenStream = "#[allow(dead_code)]".parse().unwrap();
    let mut out = allow;
    out.extend(item);
    out
}

#[proc_macro]
pub fn init(_: TokenStream) -> TokenStream {
    r#"include!(concat!(env!("OUT_DIR"), "/generated.rs"));"#
        .parse()
        .unwrap()
}
