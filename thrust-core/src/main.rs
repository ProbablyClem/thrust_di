mod services;

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

fn main() {
    println!("{:?}", GENERATED_COMPONENTS);
}
