use quote::quote;
use std::{env, fs, path::Path};
use syn::{Item, parse_file};
use walkdir::WalkDir;

fn main() {
    println!("cargo:rerun-if-changed=src/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = Path::new(&manifest_dir).join("src");

    let mut components: Vec<String> = Vec::new();

    for entry in WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
    {
        let source = match fs::read_to_string(entry.path()) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let ast = match parse_file(&source) {
            Ok(ast) => ast,
            Err(_) => continue,
        };

        for item in &ast.items {
            if let Item::Struct(s) = item {
                let has_service = s.attrs.iter().any(|attr| {
                    attr.path().get_ident().map_or(false, |id| id == "service")
                });
                if has_service {
                    components.push(s.ident.to_string());
                }
            }
        }
    }

    let component_strs: Vec<&str> = components.iter().map(String::as_str).collect();
    let generated = quote! {
        pub const GENERATED_COMPONENTS: &[&str] = &[#(#component_strs),*];
    };

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("generated.rs");
    fs::write(dest, generated.to_string()).unwrap();
}
