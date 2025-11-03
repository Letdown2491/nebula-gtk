fn main() {
    println!("cargo:rustc-check-cfg=cfg(nebula_skip_gresource)");
    if std::env::var_os("SKIP_GRESOURCE").is_some() {
        println!("cargo:rustc-cfg=nebula_skip_gresource");
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR set by Cargo");
        let out_path = std::path::Path::new(&out_dir).join("nebula.gresource");
        if !out_path.exists() {
            std::fs::write(&out_path, []).expect("create placeholder resource");
        }
        return;
    }
    glib_build_tools::compile_resources(
        &["src/resources"],
        "src/resources/nebula.gresource.xml",
        "nebula.gresource",
    );
}
