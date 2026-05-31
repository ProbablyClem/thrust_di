use quote::{format_ident, quote};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::Path,
};
use syn::{FnArg, Fields, GenericArgument, Item, PathArguments, ReturnType, Type, parse_file};
use walkdir::WalkDir;

struct ComponentInfo {
    name: String,
    deps: Vec<(String, String)>, // (field_name, type_name)
}

struct BeanInfo {
    name: String,       // return type name, unwrapped from Arc<T>
    fn_name: String,
    is_async: bool,
    deps: Vec<String>,  // dep type names (unwrapped Arc<T> from params)
    module_path: String,
}

struct LayerInfo {
    fn_name: String,
    module_path: String,
}

struct RawRouteInfo {
    fn_name: String,
    method: String,
    path: String,
    module_path: String,
    arc_params: Vec<String>,
}

struct RouteInfo {
    fn_name: String,
    method: String,
    path: String,
    module_path: String,
    service_params: Vec<String>,
}

const HTTP_METHODS: &[&str] = &["get", "post", "put", "delete", "patch"];

pub fn scan_and_generate(src_dir: &Path, out_dir: &Path) {
    let mut components: Vec<ComponentInfo> = Vec::new();
    let mut beans: Vec<BeanInfo> = Vec::new();
    let mut layers: Vec<LayerInfo> = Vec::new();
    let mut raw_routes: Vec<RawRouteInfo> = Vec::new();

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
                Item::Struct(s) => {
                    let has_service = s.attrs.iter().any(|a| {
                        a.path().get_ident().map_or(false, |id| id == "service")
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
                                let type_name = unwrap_arc(&f.ty);
                                Some((field_name, type_name))
                            })
                            .collect(),
                        _ => vec![],
                    };
                    components.push(ComponentInfo { name, deps });
                }

                Item::Fn(f) => {
                    let has_bean = f.attrs.iter().any(|a| {
                        a.path().get_ident().map_or(false, |id| id == "bean")
                    });
                    let has_layer = f.attrs.iter().any(|a| {
                        a.path().get_ident().map_or(false, |id| id == "layer")
                    });

                    if has_bean {
                        let fn_name = f.sig.ident.to_string();
                        let is_async = f.sig.asyncness.is_some();

                        let name = match &f.sig.output {
                            ReturnType::Type(_, ty) => try_unwrap_arc(ty),
                            ReturnType::Default => None,
                        }
                        .unwrap_or_else(|| {
                            panic!("thrust: `#[bean]` function `{fn_name}` must return `Arc<T>`")
                        });

                        let deps: Vec<String> = f
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
                            module_path: module_path.clone(),
                        });
                        continue;
                    }

                    if has_layer {
                        layers.push(LayerInfo {
                            fn_name: f.sig.ident.to_string(),
                            module_path: module_path.clone(),
                        });
                        continue;
                    }

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
                                panic!(
                                    "thrust: `#[{method}]` on `{fn_name}` requires a string literal path"
                                )
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
                            module_path: module_path.clone(),
                            arc_params,
                        });
                    }
                }

                _ => {}
            }
        }
    }

    // Unified known set: services + beans
    let known: HashSet<&str> = components
        .iter()
        .map(|c| c.name.as_str())
        .chain(beans.iter().map(|b| b.name.as_str()))
        .collect();

    validate_service_deps(&components, &known);

    let mut adjacency: HashMap<&str, Vec<&str>> = components
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

    for b in &beans {
        let edges: Vec<&str> = b
            .deps
            .iter()
            .filter(|d| known.contains(d.as_str()))
            .map(|d| d.as_str())
            .collect();
        adjacency.insert(b.name.as_str(), edges);
    }

    let all_names: Vec<&str> = components
        .iter()
        .map(|c| c.name.as_str())
        .chain(beans.iter().map(|b| b.name.as_str()))
        .collect();

    detect_cycles(&adjacency, &all_names);
    let order = topological_sort(&adjacency, &all_names);

    validate_routes(&raw_routes, &known);

    let routes: Vec<RouteInfo> = raw_routes
        .into_iter()
        .map(|r| RouteInfo {
            service_params: r
                .arc_params
                .iter()
                .filter(|ty| known.contains(ty.as_str()))
                .cloned()
                .collect(),
            fn_name: r.fn_name,
            method: r.method,
            path: r.path,
            module_path: r.module_path,
        })
        .collect();

    let component_map: HashMap<&str, &ComponentInfo> =
        components.iter().map(|c| (c.name.as_str(), c)).collect();
    let bean_map: HashMap<&str, &BeanInfo> =
        beans.iter().map(|b| (b.name.as_str(), b)).collect();

    // --- metadata.rs ---
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

    // --- graph.rs ---
    let graph_nodes: Vec<_> = order
        .iter()
        .map(|&name| {
            let edges = &adjacency[name];
            quote! { GraphNode { name: #name, depends_on: &[#(#edges),*] } }
        })
        .collect();

    let graph = quote! {
        pub struct GraphNode {
            pub name: &'static str,
            pub depends_on: &'static [&'static str],
        }
        pub const DEPENDENCY_GRAPH: &[GraphNode] = &[#(#graph_nodes),*];
    };

    // --- container.rs ---
    let container_fields: Vec<_> = order
        .iter()
        .map(|&name| {
            let field = format_ident!("{}", to_snake_case(name));
            let ty = format_ident!("{}", name);
            quote! { pub #field: ::std::sync::Arc<#ty> }
        })
        .collect();

    let build_stmts: Vec<_> = order
        .iter()
        .map(|&name| {
            let var = format_ident!("{}", to_snake_case(name));
            let ty = format_ident!("{}", name);

            if let Some(c) = component_map.get(name) {
                // Service: struct instantiation
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
            } else if let Some(b) = bean_map.get(name) {
                // Bean: free function call (already returns Arc<T>)
                let fn_name = format_ident!("{}", b.fn_name);

                let dep_args: Vec<_> = b
                    .deps
                    .iter()
                    .map(|dep_ty| {
                        let dep_var = format_ident!("{}", to_snake_case(dep_ty));
                        quote! { #dep_var.clone() }
                    })
                    .collect();

                let call = if b.module_path.is_empty() {
                    quote! { #fn_name(#(#dep_args),*) }
                } else {
                    let mod_tokens: proc_macro2::TokenStream =
                        b.module_path.parse().expect("valid module path");
                    quote! { #mod_tokens::#fn_name(#(#dep_args),*) }
                };

                if b.is_async {
                    quote! { let #var = #call.await; }
                } else {
                    quote! { let #var = #call; }
                }
            } else {
                panic!("thrust: unknown component `{name}` in topological order");
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

    let has_async_bean = beans.iter().any(|b| b.is_async);

    let container = if has_async_bean {
        quote! {
            pub struct Container {
                #(#container_fields),*
            }
            impl Container {
                pub async fn build() -> Self {
                    #(#build_stmts)*
                    Self { #(#self_fields),* }
                }
            }
        }
    } else {
        quote! {
            pub struct Container {
                #(#container_fields),*
            }
            impl Container {
                pub fn build() -> Self {
                    #(#build_stmts)*
                    Self { #(#self_fields),* }
                }
            }
        }
    };

    fs::write(out_dir.join("metadata.rs"), metadata.to_string()).unwrap();
    fs::write(out_dir.join("graph.rs"), graph.to_string()).unwrap();
    fs::write(out_dir.join("container.rs"), container.to_string()).unwrap();

    let mut generated = String::from(concat!(
        "include!(concat!(env!(\"OUT_DIR\"), \"/metadata.rs\"));\n",
        "include!(concat!(env!(\"OUT_DIR\"), \"/graph.rs\"));\n",
        "include!(concat!(env!(\"OUT_DIR\"), \"/container.rs\"));\n",
    ));

    if !routes.is_empty() {
        let router_ts = generate_router(&routes, &layers, has_async_bean);
        fs::write(out_dir.join("router.rs"), router_ts.to_string()).unwrap();
        generated.push_str("include!(concat!(env!(\"OUT_DIR\"), \"/router.rs\"));\n");
    }

    fs::write(out_dir.join("generated.rs"), generated).unwrap();
}

fn derive_module_path(file_path: &Path, src_dir: &Path) -> String {
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

fn generate_router(
    routes: &[RouteInfo],
    layers: &[LayerInfo],
    has_async_bean: bool,
) -> proc_macro2::TokenStream {
    let wrappers: Vec<_> = routes
        .iter()
        .map(|r| {
            let wrapper = format_ident!("__thrust_{}", r.fn_name);
            let user_fn_name = format_ident!("{}", r.fn_name);

            let (state_binding, param_clones) = if r.service_params.is_empty() {
                (
                    quote! { ::axum::extract::State(_c): ::axum::extract::State<::std::sync::Arc<Container>> },
                    vec![],
                )
            } else {
                let clones: Vec<_> = r
                    .service_params
                    .iter()
                    .map(|ty| {
                        let field = format_ident!("{}", to_snake_case(ty));
                        quote! { c.#field.clone() }
                    })
                    .collect();
                (
                    quote! { ::axum::extract::State(c): ::axum::extract::State<::std::sync::Arc<Container>> },
                    clones,
                )
            };

            let call = if r.module_path.is_empty() {
                quote! { #user_fn_name(#(#param_clones),*).await }
            } else {
                let mod_tokens: proc_macro2::TokenStream =
                    r.module_path.parse().expect("valid module path");
                quote! { #mod_tokens :: #user_fn_name(#(#param_clones),*).await }
            };

            quote! {
                async fn #wrapper(
                    #state_binding
                ) -> impl ::axum::response::IntoResponse {
                    #call
                }
            }
        })
        .collect();

    let route_calls: Vec<_> = routes
        .iter()
        .map(|r| {
            let path = &r.path;
            let method = format_ident!("{}", r.method);
            let wrapper = format_ident!("__thrust_{}", r.fn_name);
            quote! { .route(#path, ::axum::routing::#method(#wrapper)) }
        })
        .collect();

    let layer_calls: Vec<_> = layers
        .iter()
        .map(|l| {
            let fn_name = format_ident!("{}", l.fn_name);
            let call = if l.module_path.is_empty() {
                quote! { #fn_name() }
            } else {
                let mod_tokens: proc_macro2::TokenStream =
                    l.module_path.parse().expect("valid module path");
                quote! { #mod_tokens::#fn_name() }
            };
            quote! { .layer(#call) }
        })
        .collect();

    let build_call = if has_async_bean {
        quote! { Container::build().await }
    } else {
        quote! { Container::build() }
    };

    quote! {
        #(#wrappers)*

        pub fn build_router(container: ::std::sync::Arc<Container>) -> ::axum::Router {
            ::axum::Router::new()
                #(#route_calls)*
                .with_state(container)
                #(#layer_calls)*
        }

        pub async fn run() {
            let container = ::std::sync::Arc::new(#build_call);
            let port: u16 = ::std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080u16);
            let addr = ::std::format!("0.0.0.0:{}", port);
            let router = build_router(::std::sync::Arc::clone(&container));
            let listener = ::tokio::net::TcpListener::bind(&addr)
                .await
                .expect("thrust: failed to bind server address");
            ::axum::serve(listener, router)
                .await
                .expect("thrust: server error");
        }
    }
}

fn validate_service_deps(components: &[ComponentInfo], known: &HashSet<&str>) {
    let mut errors: Vec<String> = Vec::new();
    for c in components {
        for (field, ty) in &c.deps {
            if !known.contains(ty.as_str()) {
                errors.push(format!(
                    "`{}::{}` depends on `{}`, which is not a @service or @bean component",
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

fn validate_routes(raw: &[RawRouteInfo], known: &HashSet<&str>) {
    let mut errors: Vec<String> = Vec::new();
    let mut seen: HashSet<(&str, &str)> = HashSet::new();

    for r in raw {
        for ty in &r.arc_params {
            if !known.contains(ty.as_str()) {
                errors.push(format!(
                    "handler `{}` has Arc<{}> parameter but `{}` is not a @service or @bean component",
                    r.fn_name, ty, ty
                ));
            }
        }
        let key = (r.path.as_str(), r.method.as_str());
        if !seen.insert(key) {
            errors.push(format!(
                "duplicate route `{} {}` — only one handler per method+path is allowed",
                r.method.to_uppercase(),
                r.path
            ));
        }
    }

    if !errors.is_empty() {
        for e in &errors {
            eprintln!("thrust error: {e}");
        }
        panic!("thrust: invalid routes");
    }
}

fn detect_cycles<'a>(adjacency: &HashMap<&'a str, Vec<&'a str>>, names: &[&'a str]) {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_stack: HashSet<&str> = HashSet::new();
    for &name in names {
        if !visited.contains(name) {
            if let Some(cycle) = dfs(name, adjacency, &mut visited, &mut in_stack) {
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

fn topological_sort<'a>(
    adjacency: &HashMap<&'a str, Vec<&'a str>>,
    names: &[&'a str],
) -> Vec<&'a str> {
    let mut reverse: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = names.iter().map(|&n| (n, 0)).collect();
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

fn try_unwrap_arc(ty: &Type) -> Option<String> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Arc" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(type_to_string(inner));
                    }
                }
            }
        }
    }
    None
}

fn unwrap_arc(ty: &Type) -> String {
    try_unwrap_arc(ty).unwrap_or_else(|| type_to_string(ty))
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

fn type_to_string(ty: &Type) -> String {
    quote!(#ty).to_string().replace(' ', "")
}
