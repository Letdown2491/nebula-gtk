const FALLBACK_CATEGORY: &str = "Other";
const FALLBACK_ICON: &str = "/tech/geektoshi/Nebula/icons/voidlinux.png";

mod generated {
    include!(concat!(env!("OUT_DIR"), "/categories_map.rs"));
}

pub(crate) fn package_category(package: &str) -> &'static str {
    let lowercase = package.to_ascii_lowercase();
    generated::CATEGORY_MAP
        .get(lowercase.as_str())
        .copied()
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
