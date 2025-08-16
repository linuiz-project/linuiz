fn main() {
    println!("cargo::rustc-link-arg=-zmax-page-size=0x200000");

    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` must be provided");
    let target_triple = std::env::var("TARGET").expect("`TARGET` must be provided");

    println!("cargo:rustc-link-arg=-T{manifest_dir}/src/.build/{target_triple}.lds");

    println!("cargo::rerun-if-changed={manifest_dir}/src/.build/{target_triple}.a");
    println!("cargo::rustc-link-arg={manifest_dir}/src/.build/{target_triple}.a");
}
