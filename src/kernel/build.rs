fn main() {
    println!("cargo::rustc-link-arg=-zmax-page-size=0x200000");
    println!(
        "cargo:rustc-link-arg=--script={}/lds/{}.lds",
        std::env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` must be provided"),
        std::env::var("TARGET").expect("`TARGET` must be provided")
    );
}
