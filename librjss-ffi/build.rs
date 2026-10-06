fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out = std::path::Path::new(&crate_dir).join("include");
    std::fs::create_dir_all(&out).expect("failed to create include dir");

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_language(cbindgen::Language::C)
        .with_include_guard("LIBRJSS_FFI_H")
        .with_documentation(true)
        .with_cpp_compat(true)
        .generate()
        .expect("cbindgen failed to generate bindings")
        .write_to_file(out.join("librjss.h"));

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
}
