// Links the prebuilt icon resource (data/app.res, made by scripts/make-icon.ps1), so building
// needs no resource compiler.
fn main() {
    println!("cargo:rerun-if-changed=data/app.res");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let res = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("data").join("app.res");
        println!("cargo:rustc-link-arg-bins={}", res.display());
    }
}
