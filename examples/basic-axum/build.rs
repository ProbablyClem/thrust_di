fn main() {
    let src = std::path::Path::new("src");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    thrust_build::scan_and_generate(src, &out);
}
