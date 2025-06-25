fn main() {
    println!("cargo:rerun-if-changed=target/.xtraprint");
    println!(
        "cargo:rustc-link-arg=-zmax-page-size={}",
        std::env::var("KERNEL_SEGMENT_ALIGN").expect("`KERNEL_SEGMENT_ALIGN` must be provided")
    );
    println!(
        "cargo:rustc-link-arg=--script={}/lds/{}.lds",
        std::env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` must be provided"),
        std::env::var("TARGET").expect("`TARGET` must be provided")
    );
}
