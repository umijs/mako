fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rustc-env=MANIFEST_DIR={}",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );
}
