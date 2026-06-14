use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::models::{BeanInfo, ComponentInfo, LayerInfo, RouteInfo};
use crate::utils::to_snake_case;

pub fn generate_metadata(components: &[ComponentInfo]) -> TokenStream {
    let items: Vec<_> = components
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

    quote! {
        pub struct Dependency {
            pub field: &'static str,
            pub ty: &'static str,
        }
        pub struct ComponentMetadata {
            pub name: &'static str,
            pub dependencies: &'static [Dependency],
        }
        pub const GENERATED_COMPONENTS: &[ComponentMetadata] = &[#(#items),*];
    }
}

pub fn generate_graph(
    order: &[&str],
    adjacency: &HashMap<&str, Vec<&str>>,
) -> TokenStream {
    let nodes: Vec<_> = order
        .iter()
        .map(|&name| {
            let edges = &adjacency[name];
            quote! { GraphNode { name: #name, depends_on: &[#(#edges),*] } }
        })
        .collect();

    quote! {
        pub struct GraphNode {
            pub name: &'static str,
            pub depends_on: &'static [&'static str],
        }
        pub const DEPENDENCY_GRAPH: &[GraphNode] = &[#(#nodes),*];
    }
}

/// Emit a type alias per resolved trait so the `#[service]` macro can rewrite a
/// bare `dyn Trait` field into `Arc<crate::__ThrustImpl_<Trait>>` — a concrete
/// type, statically dispatched. `used` holds `(trait_name, concrete)` pairs.
pub fn generate_impl_aliases(used: &[(String, String)]) -> TokenStream {
    let aliases = used.iter().map(|(trait_name, concrete)| {
        let alias = format_ident!("__ThrustImpl_{}", trait_name);
        let ty = format_ident!("{}", concrete);
        quote! { #[allow(non_camel_case_types)] pub type #alias = #ty; }
    });
    quote! { #(#aliases)* }
}

pub fn generate_container(
    order: &[&str],
    component_map: &HashMap<&str, &ComponentInfo>,
    bean_map: &HashMap<&str, &BeanInfo>,
    has_async_bean: bool,
) -> TokenStream {
    let fields: Vec<_> = order
        .iter()
        .map(|&name| {
            let field = format_ident!("{}", to_snake_case(name));
            let ty = format_ident!("{}", name);
            quote! { pub #field: ::std::sync::Arc<#ty> }
        })
        .collect();

    let build_stmts: Vec<_> = order
        .iter()
        .map(|&name| build_stmt(name, component_map, bean_map))
        .collect();

    let self_fields: Vec<_> = order
        .iter()
        .map(|&name| {
            let f = format_ident!("{}", to_snake_case(name));
            quote! { #f }
        })
        .collect();

    if has_async_bean {
        quote! {
            pub struct Container {
                #(#fields),*
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
                #(#fields),*
            }
            impl Container {
                pub fn build() -> Self {
                    #(#build_stmts)*
                    Self { #(#self_fields),* }
                }
            }
        }
    }
}

fn build_stmt(
    name: &str,
    component_map: &HashMap<&str, &ComponentInfo>,
    bean_map: &HashMap<&str, &BeanInfo>,
) -> TokenStream {
    let var = format_ident!("{}", to_snake_case(name));
    let ty = format_ident!("{}", name);

    if let Some(c) = component_map.get(name) {
        if c.deps.is_empty() {
            return quote! { let #var = ::std::sync::Arc::new(#ty); };
        }
        let field_inits: Vec<_> = c
            .deps
            .iter()
            .map(|(field, dep_ty)| {
                let f = format_ident!("{}", field);
                let dep_var = format_ident!("{}", to_snake_case(dep_ty));
                quote! { #f: #dep_var.clone() }
            })
            .collect();
        return quote! { let #var = ::std::sync::Arc::new(#ty { #(#field_inits),* }); };
    }

    if let Some(b) = bean_map.get(name) {
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
            let mod_tokens: TokenStream = b.module_path.parse().expect("valid module path");
            quote! { #mod_tokens::#fn_name(#(#dep_args),*) }
        };
        let value = if b.is_async {
            quote! { #call.await }
        } else {
            quote! { #call }
        };
        // The container stores every component as `Arc<T>`. When the factory
        // returns a bare `T`, thrust wraps it here so users don't have to.
        let value = if b.returns_arc {
            value
        } else {
            quote! { ::std::sync::Arc::new(#value) }
        };
        return quote! { let #var = #value; };
    }

    panic!("thrust: unknown component `{name}` in topological order");
}

pub fn generate_router(
    routes: &[RouteInfo],
    layers: &[LayerInfo],
    has_async_bean: bool,
    has_server_config: bool,
) -> TokenStream {
    let wrappers: Vec<_> = routes.iter().map(generate_wrapper).collect();

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
                let mod_tokens: TokenStream =
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

    // Server config comes from a `#[bean] fn ... -> Arc<ServerConfig>` when the
    // user provides one (exposed on the container as `server_config`), otherwise
    // we fall back to `ServerConfig::default()` (`PORT` env / 8080 on 0.0.0.0).
    let config_binding = if has_server_config {
        quote! { let config = ::std::sync::Arc::clone(&container.server_config); }
    } else {
        quote! { let config = ::std::sync::Arc::new(ServerConfig::default()); }
    };

    quote! {
        /// Server bind configuration. Override by declaring a thrust `#[bean]`
        /// that returns `::std::sync::Arc<ServerConfig>`; otherwise `Default`
        /// is used (`PORT` env var or 8080, bound on `0.0.0.0`).
        #[derive(Clone, Debug)]
        pub struct ServerConfig {
            pub host: ::std::string::String,
            pub port: u16,
        }

        impl ::std::default::Default for ServerConfig {
            fn default() -> Self {
                let port: u16 = ::std::env::var("PORT")
                    .ok()
                    .and_then(|p| p.parse().ok())
                    .unwrap_or(8080u16);
                ServerConfig { host: ::std::string::String::from("0.0.0.0"), port }
            }
        }

        #(#wrappers)*

        pub fn build_router(container: ::std::sync::Arc<Container>) -> ::axum::Router {
            ::axum::Router::new()
                #(#route_calls)*
                .with_state(container)
                #(#layer_calls)*
        }

        pub async fn run() {
            let container = ::std::sync::Arc::new(#build_call);
            #config_binding
            let addr = ::std::format!("{}:{}", config.host, config.port);
            let router = build_router(::std::sync::Arc::clone(&container));
            let listener = ::tokio::net::TcpListener::bind(&addr)
                .await
                .expect("thrust: failed to bind server address");
            println!("thrust: listening on http://{}", addr);
            ::axum::serve(listener, router)
                .await
                .expect("thrust: server error");
        }
    }
}

fn generate_wrapper(r: &RouteInfo) -> TokenStream {
    let wrapper = format_ident!("__thrust_{}", r.fn_name);
    let user_fn = format_ident!("{}", r.fn_name);

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
        quote! { #user_fn(#(#param_clones),*).await }
    } else {
        let mod_tokens: TokenStream = r.module_path.parse().expect("valid module path");
        quote! { #mod_tokens::#user_fn(#(#param_clones),*).await }
    };

    quote! {
        async fn #wrapper(
            #state_binding
        ) -> impl ::axum::response::IntoResponse {
            #call
        }
    }
}
