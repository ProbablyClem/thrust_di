use quote::{format_ident, quote};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env, fs,
    path::Path,
};
use syn::{Fields, GenericArgument, Item, PathArguments, Type, parse_file};
use walkdir::WalkDir;

struct ComponentInfo {
    name: String,
    deps: Vec<(String, String)>, // (field_name, type_name)
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
                            // Unwrap Arc<T> → T for component resolution
                            let type_name = unwrap_arc(&f.ty);
                            Some((field_name, type_name))
                        })
                        .collect(),
                    _ => vec![],
                };

                components.push(ComponentInfo { name, deps });
            }
        }
    }

    let known: HashSet<&str> = components.iter().map(|c| c.name.as_str()).collect();

    // Phase 4: all dep types must be known components
    validate_deps(&components, &known);

    // Adjacency: name -> dep type names (only known components)
    let adjacency: HashMap<&str, Vec<&str>> = components
        .iter()
        .map(|c| {
            let edges: Vec<&str> = c
                .deps
                .iter()
                .map(|(_, ty)| ty.as_str())
                .filter(|ty| known.contains(*ty))
                .collect();
            (c.name.as_str(), edges)
        })
        .collect();

    // Phase 5: no cycles allowed
    detect_cycles(&adjacency, &components);

    // Phase 6: topological sort → construction order (deps before dependents)
    let order = topological_sort(&adjacency, &components);

    // Phase 2: full metadata (field name + type)
    let metadata_items: Vec<_> = components
        .iter()
        .map(|c| {
            let name = &c.name;
            let dep_entries = c.deps.iter().map(|(field, ty)| {
                quote! { Dependency { field: #field, ty: #ty } }
            });
            quote! {
                ComponentMetadata { name: #name, dependencies: &[#(#dep_entries),*] }
            }
        })
        .collect();

    // Phase 3: adjacency graph in topological order
    let graph_nodes: Vec<_> = order
        .iter()
        .map(|&name| {
            let edges = &adjacency[name];
            quote! { GraphNode { name: #name, depends_on: &[#(#edges),*] } }
        })
        .collect();

    // Phase 7: Container struct fields (Arc<T> per component, in topo order)
    let container_fields: Vec<_> = order
        .iter()
        .map(|&name| {
            let field = format_ident!("{}", to_snake_case(name));
            let ty = format_ident!("{}", name);
            quote! { pub #field: ::std::sync::Arc<#ty> }
        })
        .collect();

    // Phase 8: Container::build() — construct in topo order, clone Arcs for deps
    let build_stmts: Vec<_> = order
        .iter()
        .map(|&name| {
            let var = format_ident!("{}", to_snake_case(name));
            let ty = format_ident!("{}", name);
            let c = components.iter().find(|c| c.name == name).unwrap();

            if c.deps.is_empty() {
                quote! { let #var = ::std::sync::Arc::new(#ty); }
            } else {
                let field_inits: Vec<_> = c
                    .deps
                    .iter()
                    .map(|(field, dep_ty)| {
                        let f = format_ident!("{}", field);
                        let dep_var = format_ident!("{}", to_snake_case(dep_ty));
                        quote! { #f: #dep_var.clone() }
                    })
                    .collect();
                quote! { let #var = ::std::sync::Arc::new(#ty { #(#field_inits),* }); }
            }
        })
        .collect();

    let self_fields: Vec<_> = order
        .iter()
        .map(|&name| {
            let f = format_ident!("{}", to_snake_case(name));
            quote! { #f }
        })
        .collect();

    let out_dir = env::var("OUT_DIR").unwrap();
    let out = Path::new(&out_dir);

    let metadata = quote! {
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

    let graph = quote! {
        pub struct GraphNode {
            pub name: &'static str,
            pub depends_on: &'static [&'static str],
        }

        pub const DEPENDENCY_GRAPH: &[GraphNode] = &[#(#graph_nodes),*];
    };

    let container = quote! {
        pub struct Container {
            #(#container_fields),*
        }

        impl Container {
            pub fn build() -> Self {
                #(#build_stmts)*
                Self { #(#self_fields),* }
            }
        }
    };

    fs::write(out.join("metadata.rs"), metadata.to_string()).unwrap();
    fs::write(out.join("graph.rs"), graph.to_string()).unwrap();
    fs::write(out.join("container.rs"), container.to_string()).unwrap();
    fs::write(
        out.join("generated.rs"),
        r#"include!(concat!(env!("OUT_DIR"), "/metadata.rs"));
include!(concat!(env!("OUT_DIR"), "/graph.rs"));
include!(concat!(env!("OUT_DIR"), "/container.rs"));
"#,
    )
    .unwrap();
}

// Phase 4: abort if any field type is not a registered component
fn validate_deps(components: &[ComponentInfo], known: &HashSet<&str>) {
    let mut errors: Vec<String> = Vec::new();
    for c in components {
        for (field, ty) in &c.deps {
            if !known.contains(ty.as_str()) {
                errors.push(format!(
                    "`{}::{}` depends on `{}`, which is not a @service component",
                    c.name, field, ty
                ));
            }
        }
    }
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("thrust error: {e}");
        }
        panic!("thrust: unresolved dependencies");
    }
}

// Phase 5: DFS cycle detection with gray/black coloring
fn detect_cycles<'a>(adjacency: &HashMap<&'a str, Vec<&'a str>>, components: &'a [ComponentInfo]) {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_stack: HashSet<&str> = HashSet::new();

    for c in components {
        if !visited.contains(c.name.as_str()) {
            if let Some(cycle) = dfs(c.name.as_str(), adjacency, &mut visited, &mut in_stack) {
                panic!("thrust: dependency cycle detected: {}", cycle.join(" -> "));
            }
        }
    }
}

fn dfs<'a>(
    node: &'a str,
    adjacency: &HashMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    in_stack: &mut HashSet<&'a str>,
) -> Option<Vec<String>> {
    visited.insert(node);
    in_stack.insert(node);

    if let Some(neighbors) = adjacency.get(node) {
        for &neighbor in neighbors {
            if in_stack.contains(neighbor) {
                return Some(vec![neighbor.to_string(), node.to_string()]);
            }
            if !visited.contains(neighbor) {
                if let Some(mut path) = dfs(neighbor, adjacency, visited, in_stack) {
                    path.push(node.to_string());
                    return Some(path);
                }
            }
        }
    }

    in_stack.remove(node);
    None
}

// Phase 6: Kahn's algorithm on the reversed graph (deps before dependents)
fn topological_sort<'a>(
    adjacency: &HashMap<&'a str, Vec<&'a str>>,
    components: &'a [ComponentInfo],
) -> Vec<&'a str> {
    let mut reverse: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> =
        components.iter().map(|c| (c.name.as_str(), 0)).collect();

    for (&node, deps) in adjacency {
        for &dep in deps {
            reverse.entry(dep).or_default().push(node);
            *in_degree.entry(node).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(name, _)| *name)
        .collect();

    let mut result: Vec<&str> = Vec::new();

    while let Some(node) = queue.pop_front() {
        result.push(node);
        if let Some(successors) = reverse.get(node) {
            for &s in successors {
                let deg = in_degree.get_mut(s).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(s);
                }
            }
        }
    }

    result
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.char_indices() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}

// Strip Arc<T> → T so the framework resolves the component, not the wrapper
fn unwrap_arc(ty: &Type) -> String {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Arc" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return type_to_string(inner);
                    }
                }
            }
        }
    }
    type_to_string(ty)
}

fn type_to_string(ty: &Type) -> String {
    quote!(#ty).to_string().replace(' ', "")
}
