use quote::quote;
use std::{env, fs, path::Path};
use syn::{Fields, Item, Type, parse_file};
use walkdir::WalkDir;

struct ComponentInfo {
    name: String,
    deps: Vec<(String, String)>,
}

fn main() {
    println!("cargo:rerun-if-changed=src/");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = Path::new(&manifest_dir).join("src");

    let mut components: Vec<ComponentInfo> = Vec::new();

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
                if !has_service {
                    continue;
                }

                let name = s.ident.to_string();
                let deps = match &s.fields {
                    Fields::Named(named) => named
                        .named
                        .iter()
                        .filter_map(|f| {
                            let field_name = f.ident.as_ref()?.to_string();
                            let type_name = type_to_string(&f.ty);
                            Some((field_name, type_name))
                        })
                        .collect(),
                    _ => vec![],
                };

                components.push(ComponentInfo { name, deps });
            }
        }
    }

    let metadata_items = components.iter().map(|c| {
        let name = &c.name;
        let dep_entries = c.deps.iter().map(|(field, ty)| {
            quote! { Dependency { field: #field, ty: #ty } }
        });
        quote! {
            ComponentMetadata {
                name: #name,
                dependencies: &[#(#dep_entries),*],
            }
        }
    });

    let generated = quote! {
        pub struct Dependency {
            pub field: &'static str,
            pub ty: &'static str,
        }

        pub struct ComponentMetadata {
            pub name: &'static str,
            pub dependencies: &'static [Dependency],
        }

        pub const GENERATED_COMPONENTS: &[ComponentMetadata] = &[#(#metadata_items),*];
    };

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("generated.rs");
    fs::write(dest, generated.to_string()).unwrap();
}

fn type_to_string(ty: &Type) -> String {
    quote!(#ty).to_string().replace(' ', "")
}
