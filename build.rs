fn main() {
    if std::env::var_os("SKIP_GRESOURCE").is_some() {
        return;
    }
    glib_build_tools::compile_resources(
        &["src/resources"],
        "src/resources/nebula.gresource.xml",
        "nebula.gresource",
    );
}
