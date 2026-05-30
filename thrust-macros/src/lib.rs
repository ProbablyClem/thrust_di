use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn service(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let allow: TokenStream = "#[allow(dead_code)]".parse().unwrap();
    let mut out = allow;
    out.extend(item);
    out
}
