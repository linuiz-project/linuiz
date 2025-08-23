fn main() {
    let target_triple = std::env::var("TARGET").expect("`TARGET` must be provided");

    println!("cargo::rerun-if-changed=build/{target_triple}.a");

    println!("cargo::rustc-link-arg=-zmax-page-size=0x200000");
    println!("cargo::rustc-link-arg=build/{target_triple}.a");
    println!("cargo:rustc-link-arg=-Tbuild/{target_triple}.lds");
}
