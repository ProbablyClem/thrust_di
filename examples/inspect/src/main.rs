mod services;

use services::*;

thrust_macros::init!();

fn main() {
    println!("=== components ===");
    for c in GENERATED_COMPONENTS {
        if c.dependencies.is_empty() {
            println!("  {}: no dependencies", c.name);
        } else {
            for dep in c.dependencies {
                println!("  {}: {} -> {}", c.name, dep.field, dep.ty);
            }
        }
    }

    println!("\n=== dependency graph (construction order) ===");
    for node in DEPENDENCY_GRAPH {
        if node.depends_on.is_empty() {
            println!("  {} -> []", node.name);
        } else {
            println!("  {} -> [{}]", node.name, node.depends_on.join(", "));
        }
    }
}
