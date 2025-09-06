fn main() {
    let target_triple = std::env::var("TARGET").expect("`TARGET` must be provided");



    println!("cargo::rerun-if-changed=build/{target_triple}.a");

    // .
}
