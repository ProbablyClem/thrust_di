use std::{env, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=src/");
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out = env::var("OUT_DIR").unwrap();
    thrust_build::scan_and_generate(&Path::new(&manifest).join("src"), Path::new(&out));
}
