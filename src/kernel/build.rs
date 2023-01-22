fn main() {
    println!(
        "cargo:rustc-link-arg=--script={}/lds/{}.lds",
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        std::env::var("TARGET").unwrap()
    );
}
