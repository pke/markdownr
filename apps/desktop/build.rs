// Windows: links the prebuilt icon resource (data/app.res, made by scripts/make-icon.ps1), so
// building needs no resource compiler.
// Linux: links the system libraries by their runtime names (libcairo.so.2, …), so building
// needs no -dev packages; --as-needed drops any the binary does not use.
fn main() {
    println!("cargo:rerun-if-changed=data/app.res");
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("windows") => {
            let res = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("data").join("app.res");
            println!("cargo:rustc-link-arg-bins={}", res.display());
        }
        Ok("linux") => {
            println!("cargo:rustc-link-arg-bins=-Wl,--as-needed");
            for lib in [
                "libX11.so.6",
                "libcairo.so.2",
                "libpango-1.0.so.0",
                "libpangocairo-1.0.so.0",
                "libgobject-2.0.so.0",
                "libglib-2.0.so.0",
                "libgdk_pixbuf-2.0.so.0",
                "libfontconfig.so.1",
                "libz.so.1",
            ] {
                println!("cargo:rustc-link-arg-bins=-l:{lib}");
            }
        }
        _ => {}
    }
}
