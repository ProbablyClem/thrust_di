use std::path::Path;
use syn::{GenericArgument, PathArguments, Type, TypeParamBound};

pub fn derive_module_path(file_path: &Path, src_dir: &Path) -> String {
    let rel = file_path.strip_prefix(src_dir).unwrap_or(file_path);
    let without_ext = rel.with_extension("");
    let parts: Vec<_> = without_ext
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let joined = parts.join("::");
    if joined == "main" {
        String::new()
    } else {
        format!("crate::{joined}")
    }
}

pub fn try_unwrap_arc(ty: &Type) -> Option<String> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Arc" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(type_to_dep_name(inner));
                    }
                }
            }
        }
    }
    None
}

/// Name used to identify a dependency. For a trait object `dyn Trait + ...`,
/// this is the bare trait name (`Trait`) so it can be resolved to the concrete
/// `#[service]` that implements it. For any other type, it is the type itself.
fn type_to_dep_name(ty: &Type) -> String {
    if let Type::TraitObject(obj) = ty {
        for bound in &obj.bounds {
            if let TypeParamBound::Trait(tb) = bound {
                if let Some(seg) = tb.path.segments.last() {
                    return seg.ident.to_string();
                }
            }
        }
    }
    type_to_string(ty)
}

pub fn unwrap_arc(ty: &Type) -> String {
    try_unwrap_arc(ty).unwrap_or_else(|| type_to_dep_name(ty))
}

pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.char_indices() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

pub fn type_to_string(ty: &Type) -> String {
    quote::quote!(#ty).to_string().replace(' ', "")
}
