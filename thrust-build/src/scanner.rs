use std::{fs, path::Path};
use syn::{FnArg, Fields, Item, ReturnType, parse_file};
use walkdir::WalkDir;

use crate::models::{BeanInfo, ComponentInfo, LayerInfo, RawRouteInfo, TraitImpl, HTTP_METHODS};
use crate::utils::{derive_module_path, try_unwrap_arc, type_to_string, unwrap_arc};

pub fn scan_source(
    src_dir: &Path,
) -> (
    Vec<ComponentInfo>,
    Vec<BeanInfo>,
    Vec<LayerInfo>,
    Vec<RawRouteInfo>,
    Vec<TraitImpl>,
) {
    let mut components: Vec<ComponentInfo> = Vec::new();
    let mut beans: Vec<BeanInfo> = Vec::new();
    let mut layers: Vec<LayerInfo> = Vec::new();
    let mut raw_routes: Vec<RawRouteInfo> = Vec::new();
    let mut trait_impls: Vec<TraitImpl> = Vec::new();

    for entry in WalkDir::new(src_dir)
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

        let module_path = derive_module_path(entry.path(), src_dir);

        for item in &ast.items {
            match item {
                Item::Struct(s) => collect_service(s, &mut components),
                Item::Impl(i) => collect_impl(i, &mut trait_impls),
                Item::Fn(f) => {
                    let _ = collect_bean(f, &module_path, &mut beans)
                        || collect_layer(f, &module_path, &mut layers)
                        || collect_routes(f, &module_path, &mut raw_routes);
                }
                _ => {}
            }
        }
    }

    (components, beans, layers, raw_routes, trait_impls)
}

/// Record `impl Trait for Concrete` blocks so trait-typed dependencies
/// (`Arc<dyn Trait>`) can be resolved to the concrete component that
/// implements them. Inherent impls (`impl Concrete { .. }`) are ignored.
fn collect_impl(i: &syn::ItemImpl, trait_impls: &mut Vec<TraitImpl>) {
    let Some((_, trait_path, _)) = &i.trait_ else {
        return;
    };
    let Some(trait_name) = trait_path.segments.last().map(|s| s.ident.to_string()) else {
        return;
    };
    let concrete = type_to_string(&i.self_ty);
    trait_impls.push(TraitImpl {
        trait_name,
        concrete,
    });
}

fn collect_service(s: &syn::ItemStruct, components: &mut Vec<ComponentInfo>) {
    let has_service = s
        .attrs
        .iter()
        .any(|a| a.path().get_ident().map_or(false, |id| id == "service"));
    if !has_service {
        return;
    }

    let name = s.ident.to_string();
    let deps = match &s.fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|f| {
                let field_name = f.ident.as_ref()?.to_string();
                let type_name = unwrap_arc(&f.ty);
                Some((field_name, type_name))
            })
            .collect(),
        _ => vec![],
    };
    components.push(ComponentInfo { name, deps });
}

fn collect_bean(f: &syn::ItemFn, module_path: &str, beans: &mut Vec<BeanInfo>) -> bool {
    let has_bean = f
        .attrs
        .iter()
        .any(|a| a.path().get_ident().map_or(false, |id| id == "bean"));
    if !has_bean {
        return false;
    }

    let fn_name = f.sig.ident.to_string();
    let is_async = f.sig.asyncness.is_some();

    let name = match &f.sig.output {
        ReturnType::Type(_, ty) => try_unwrap_arc(ty),
        ReturnType::Default => None,
    }
    .unwrap_or_else(|| panic!("thrust: `#[bean]` function `{fn_name}` must return `Arc<T>`"));

    let deps = f
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pt) = arg {
                try_unwrap_arc(&pt.ty)
            } else {
                None
            }
        })
        .collect();

    beans.push(BeanInfo {
        name,
        fn_name,
        is_async,
        deps,
        module_path: module_path.to_owned(),
    });
    true
}

fn collect_layer(f: &syn::ItemFn, module_path: &str, layers: &mut Vec<LayerInfo>) -> bool {
    let has_layer = f
        .attrs
        .iter()
        .any(|a| a.path().get_ident().map_or(false, |id| id == "layer"));
    if !has_layer {
        return false;
    }
    layers.push(LayerInfo {
        fn_name: f.sig.ident.to_string(),
        module_path: module_path.to_owned(),
    });
    true
}

fn collect_routes(
    f: &syn::ItemFn,
    module_path: &str,
    raw_routes: &mut Vec<RawRouteInfo>,
) -> bool {
    let mut found = false;
    for attr in &f.attrs {
        let method = attr
            .path()
            .get_ident()
            .map(|id| id.to_string())
            .filter(|m| HTTP_METHODS.contains(&m.as_str()));
        let Some(method) = method else { continue };

        let fn_name = f.sig.ident.to_string();

        if f.sig.asyncness.is_none() {
            panic!("thrust: `#[{method}]` on `{fn_name}` must be async");
        }

        let path = attr
            .parse_args::<syn::LitStr>()
            .unwrap_or_else(|_| {
                panic!("thrust: `#[{method}]` on `{fn_name}` requires a string literal path")
            })
            .value();

        let arc_params = f
            .sig
            .inputs
            .iter()
            .filter_map(|arg| {
                if let FnArg::Typed(pt) = arg {
                    try_unwrap_arc(&pt.ty)
                } else {
                    None
                }
            })
            .collect();

        raw_routes.push(RawRouteInfo {
            fn_name,
            method,
            path,
            module_path: module_path.to_owned(),
            arc_params,
        });
        found = true;
    }
    found
}
