use quote::quote;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env, fs,
    path::Path,
};
use syn::{Fields, Item, Type, parse_file};
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

    // Emit generated.rs
    let metadata_items = components.iter().map(|c| {
        let name = &c.name;
        let dep_entries = c.deps.iter().map(|(field, ty)| {
            quote! { Dependency { field: #field, ty: #ty } }
        });
        quote! {
            ComponentMetadata { name: #name, dependencies: &[#(#dep_entries),*] }
        }
    });

    let graph_nodes = order.iter().map(|&name| {
        let edges = &adjacency[name];
        quote! {
            GraphNode { name: #name, depends_on: &[#(#edges),*] }
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

        pub struct GraphNode {
            pub name: &'static str,
            pub depends_on: &'static [&'static str],
        }

        pub const GENERATED_COMPONENTS: &[ComponentMetadata] = &[#(#metadata_items),*];

        pub const DEPENDENCY_GRAPH: &[GraphNode] = &[#(#graph_nodes),*];
    };

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("generated.rs");
    fs::write(dest, generated.to_string()).unwrap();
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
    // reverse: dep -> [nodes that depend on dep]
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

fn type_to_string(ty: &Type) -> String {
    quote!(#ty).to_string().replace(' ', "")
}
