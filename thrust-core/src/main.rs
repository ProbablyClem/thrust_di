mod services;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

fn main() {
    for c in GENERATED_COMPONENTS {
        if c.dependencies.is_empty() {
            println!("{}: no dependencies", c.name);
        } else {
            for dep in c.dependencies {
                println!("{}: {} -> {}", c.name, dep.field, dep.ty);
            }
        }
    }
}
