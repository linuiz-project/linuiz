fn main() {
    println!("cargo:rustc-link-args=-code-model=kernel");
    println!("cargo:rustc-link-args=-T./lds/{}.lds", std::env::var_os("TARGET").unwrap().into_string().unwrap());
}
