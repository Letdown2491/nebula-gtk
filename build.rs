use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
struct CategorySuggestions {
    packages: Vec<CategoryEntry>,
}

#[derive(Deserialize)]
struct CategoryEntry {
    pkgname: String,
    category: String,
}

fn main() {
    println!("cargo:rustc-check-cfg=cfg(nebula_skip_gresource)");
    generate_category_map();

    if env::var_os("SKIP_GRESOURCE").is_some() {
        println!("cargo:rustc-cfg=nebula_skip_gresource");
        let out_dir = env::var("OUT_DIR").expect("OUT_DIR set by Cargo");
        let out_path = Path::new(&out_dir).join("nebula.gresource");
        if !out_path.exists() {
            fs::write(&out_path, []).expect("create placeholder resource");
        }
        return;
    }

    glib_build_tools::compile_resources(
        &["src/resources"],
        "src/resources/nebula.gresource.xml",
        "nebula.gresource",
    );
}

fn generate_category_map() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR set by Cargo"));
    let dest_path = out_dir.join("categories_map.rs");

    let suggestions_path = Path::new("data/generated/category_suggestions.json");
    let suggestions = fs::read_to_string(suggestions_path);

    let mut file = File::create(&dest_path).expect("create categories_map.rs");

    let mut map_builder = phf_codegen::Map::new();
    if let Ok(raw) = suggestions {
        match serde_json::from_str::<CategorySuggestions>(&raw) {
            Ok(data) => {
                let mut seen = HashSet::new();
                for entry in data.packages {
                    let pkg = entry.pkgname.trim();
                    let category = entry.category.trim();
                    if pkg.is_empty() || category.is_empty() {
                        continue;
                    }
                    let key = pkg.to_ascii_lowercase();
                    if !seen.insert(key.clone()) {
                        continue;
                    }
                    map_builder.entry(key, format!("{category:?}"));
                }
            }
            Err(err) => {
                eprintln!("Failed to parse category suggestions: {}", err);
            }
        }
    } else if let Err(err) = suggestions {
        eprintln!(
            "Failed to read category suggestions file {}: {}",
            suggestions_path.display(),
            err
        );
    }

    writeln!(
        &mut file,
        "pub(super) static CATEGORY_MAP: phf::Map<&'static str, &'static str> = {};",
        map_builder.build()
    )
    .expect("write categories_map.rs");
}
