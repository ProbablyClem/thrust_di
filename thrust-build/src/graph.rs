use std::collections::{HashMap, HashSet, VecDeque};

use crate::models::{BeanInfo, ComponentInfo, RawRouteInfo, RouteInfo, TraitImpl};

/// Rewrite trait-typed dependencies (recorded under the bare trait name) to the
/// concrete component that implements the trait. This lets services depend on an
/// abstraction while the container instantiates a concrete type and wires it
/// into the (always trait-object) field. A trait with more than one implementing
/// component is ambiguous and aborts the build.
pub fn resolve_trait_deps(
    trait_impls: &[TraitImpl],
    components: &mut [ComponentInfo],
    beans: &mut [BeanInfo],
    raw_routes: &mut [RawRouteInfo],
) {
    let mut impls: HashMap<&str, Vec<&str>> = HashMap::new();
    for ti in trait_impls {
        impls
            .entry(ti.trait_name.as_str())
            .or_default()
            .push(ti.concrete.as_str());
    }

    let resolve = |ty: &str| -> Option<String> {
        let concretes = impls.get(ty)?;
        if concretes.len() > 1 {
            panic!(
                "thrust: trait `{ty}` is implemented by multiple components ({}); \
                 cannot decide which to inject for `Arc<dyn {ty}>`",
                concretes.join(", ")
            );
        }
        Some(concretes[0].to_string())
    };

    for c in components.iter_mut() {
        for (_, ty) in c.deps.iter_mut() {
            if let Some(concrete) = resolve(ty) {
                *ty = concrete;
            }
        }
    }
    for b in beans.iter_mut() {
        for ty in b.deps.iter_mut() {
            if let Some(concrete) = resolve(ty) {
                *ty = concrete;
            }
        }
    }
    for r in raw_routes.iter_mut() {
        for ty in r.arc_params.iter_mut() {
            if let Some(concrete) = resolve(ty) {
                *ty = concrete;
            }
        }
    }
}

pub fn build_adjacency<'a>(
    components: &'a [ComponentInfo],
    beans: &'a [BeanInfo],
    known: &HashSet<&str>,
) -> HashMap<&'a str, Vec<&'a str>> {
    let mut adjacency: HashMap<&str, Vec<&str>> = components
        .iter()
        .map(|c| {
            let edges = c
                .deps
                .iter()
                .map(|(_, ty)| ty.as_str())
                .filter(|ty| known.contains(*ty))
                .collect();
            (c.name.as_str(), edges)
        })
        .collect();

    for b in beans {
        let edges = b
            .deps
            .iter()
            .filter(|d| known.contains(d.as_str()))
            .map(|d| d.as_str())
            .collect();
        adjacency.insert(b.name.as_str(), edges);
    }

    adjacency
}

pub fn resolve_routes(raw_routes: Vec<RawRouteInfo>, known: &HashSet<&str>) -> Vec<RouteInfo> {
    raw_routes
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
        .collect()
}

pub fn validate_service_deps(components: &[ComponentInfo], known: &HashSet<&str>) {
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

pub fn validate_routes(raw: &[RawRouteInfo], known: &HashSet<&str>) {
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

pub fn detect_cycles<'a>(adjacency: &HashMap<&'a str, Vec<&'a str>>, names: &[&'a str]) {
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

pub fn topological_sort<'a>(
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
