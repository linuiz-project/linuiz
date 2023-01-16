fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let target = std::env::var("TARGET").unwrap();
    println!("cargo:rustc-link-arg=--script={manifest_dir}/lds/{target}.lds");
}
