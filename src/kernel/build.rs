fn main() {
    if let Ok(kernel_segment_align) = std::env::var("KERNEL_SEGMENT_ALIGN") {
        println!("cargo:rustc-link-arg=-zmax-page-size={kernel_segment_align}");
    }

    println!(
        "cargo:rustc-link-arg=--script={}/lds/{}.lds",
        std::env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` must be provided"),
        std::env::var("TARGET").expect("`TARGET` must be provided")
    );
}
