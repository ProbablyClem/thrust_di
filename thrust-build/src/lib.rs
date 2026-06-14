mod codegen;
mod graph;
mod models;
mod scanner;
mod utils;

use std::{collections::HashMap, fs, path::Path};

pub fn scan_and_generate(src_dir: &Path, out_dir: &Path) {
    let (mut components, mut beans, layers, mut raw_routes, trait_impls) =
        scanner::scan_source(src_dir);

    graph::resolve_trait_deps(&trait_impls, &mut components, &mut beans, &mut raw_routes);

    let known: std::collections::HashSet<&str> = components
        .iter()
        .map(|c| c.name.as_str())
        .chain(beans.iter().map(|b| b.name.as_str()))
        .collect();

    graph::validate_service_deps(&components, &known);

    let adjacency = graph::build_adjacency(&components, &beans, &known);
    let all_names: Vec<&str> = components
        .iter()
        .map(|c| c.name.as_str())
        .chain(beans.iter().map(|b| b.name.as_str()))
        .collect();

    graph::detect_cycles(&adjacency, &all_names);
    let order = graph::topological_sort(&adjacency, &all_names);

    graph::validate_routes(&raw_routes, &known);
    let routes = graph::resolve_routes(raw_routes, &known);

    let component_map: HashMap<&str, &models::ComponentInfo> =
        components.iter().map(|c| (c.name.as_str(), c)).collect();
    let bean_map: HashMap<&str, &models::BeanInfo> =
        beans.iter().map(|b| (b.name.as_str(), b)).collect();

    let has_async_bean = beans.iter().any(|b| b.is_async);
    let has_server_config = beans.iter().any(|b| b.name == "ServerConfig");

    let metadata = codegen::generate_metadata(&components);
    let graph = codegen::generate_graph(&order, &adjacency);
    let container = codegen::generate_container(&order, &component_map, &bean_map, has_async_bean);

    fs::write(out_dir.join("metadata.rs"), metadata.to_string()).unwrap();
    fs::write(out_dir.join("graph.rs"), graph.to_string()).unwrap();
    fs::write(out_dir.join("container.rs"), container.to_string()).unwrap();

    let mut generated = String::from(concat!(
        "include!(concat!(env!(\"OUT_DIR\"), \"/metadata.rs\"));\n",
        "include!(concat!(env!(\"OUT_DIR\"), \"/graph.rs\"));\n",
        "include!(concat!(env!(\"OUT_DIR\"), \"/container.rs\"));\n",
    ));

    if !routes.is_empty() {
        let router_ts =
            codegen::generate_router(&routes, &layers, has_async_bean, has_server_config);
        fs::write(out_dir.join("router.rs"), router_ts.to_string()).unwrap();
        generated.push_str("include!(concat!(env!(\"OUT_DIR\"), \"/router.rs\"));\n");
    }

    fs::write(out_dir.join("generated.rs"), generated).unwrap();
}
