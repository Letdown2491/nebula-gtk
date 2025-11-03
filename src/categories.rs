use std::collections::HashMap;

use once_cell::sync::Lazy;
use serde::Deserialize;

const FALLBACK_CATEGORY: &str = "Other";
const FALLBACK_ICON: &str = "/tech/geektoshi/Nebula/icons/voidlinux.png";

#[derive(Deserialize)]
struct SuggestionFile {
    packages: Vec<PackageRecord>,
}

#[derive(Deserialize)]
struct PackageRecord {
    pkgname: String,
    category: String,
}

struct CategoryIndex {
    package_to_category: HashMap<String, Box<str>>,
}

impl CategoryIndex {
    fn load() -> Self {
        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/generated/category_suggestions.json"
        ));

        match serde_json::from_str::<SuggestionFile>(raw) {
            Ok(file) => {
                let mut map = HashMap::with_capacity(file.packages.len());
                for entry in file.packages {
                    if entry.pkgname.is_empty() || entry.category.is_empty() {
                        continue;
                    }
                    map.insert(entry.pkgname.to_ascii_lowercase(), entry.category.into());
                }
                Self {
                    package_to_category: map,
                }
            }
            Err(err) => {
                eprintln!("Failed to parse category suggestions: {}", err);
                Self {
                    package_to_category: HashMap::new(),
                }
            }
        }
    }

    fn category_for(&self, package: &str) -> Option<&str> {
        self.package_to_category
            .get(&package.to_ascii_lowercase())
            .map(|value| value.as_ref())
    }
}

static CATEGORY_INDEX: Lazy<CategoryIndex> = Lazy::new(CategoryIndex::load);

pub(crate) fn package_category(package: &str) -> &'static str {
    CATEGORY_INDEX
        .category_for(package)
        .unwrap_or(FALLBACK_CATEGORY)
}

pub(crate) fn icon_resource_for_package(package: &str) -> &'static str {
    let category = package_category(package);
    icon_resource_for_category(category)
}

pub(crate) fn icon_resource_for_category(category: &str) -> &'static str {
    match category {
        "Books" => "/tech/geektoshi/Nebula/icons/books.svg",
        "Browsers" => "/tech/geektoshi/Nebula/icons/browsers.svg",
        "Chat" => "/tech/geektoshi/Nebula/icons/chat.svg",
        "Development" => "/tech/geektoshi/Nebula/icons/development.svg",
        "Education" => "/tech/geektoshi/Nebula/icons/education.svg",
        "E-mail" => "/tech/geektoshi/Nebula/icons/email.svg",
        "Finance" => "/tech/geektoshi/Nebula/icons/finance.svg",
        "Gaming" => "/tech/geektoshi/Nebula/icons/games.svg",
        "Graphics" => "/tech/geektoshi/Nebula/icons/graphics.svg",
        "Kernels" => "/tech/geektoshi/Nebula/icons/kernels.svg",
        "Music" => "/tech/geektoshi/Nebula/icons/music.svg",
        "News" => "/tech/geektoshi/Nebula/icons/news.svg",
        "Office" => "/tech/geektoshi/Nebula/icons/office.svg",
        "Photos" => "/tech/geektoshi/Nebula/icons/photo.svg",
        "Productivity" => "/tech/geektoshi/Nebula/icons/productivity.svg",
        "System" => "/tech/geektoshi/Nebula/icons/system.svg",
        "Tools and Utilities" => "/tech/geektoshi/Nebula/icons/tools.svg",
        "Video" => "/tech/geektoshi/Nebula/icons/video.svg",
        "Other" => FALLBACK_ICON,
        _ => FALLBACK_ICON,
    }
}
