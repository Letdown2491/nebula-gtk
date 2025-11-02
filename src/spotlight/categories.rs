#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SpotlightCategory {
    Browsers,
    Chat,
    Games,
    Email,
    Productivity,
    Utilities,
    Graphics,
    Music,
    Video,
}

pub(crate) fn category_allowlist(category: SpotlightCategory) -> &'static [&'static str] {
    match category {
        SpotlightCategory::Browsers => &[
            "firefox",
            "chromium",
            "ungoogled-chromium",
            "falkon",
            "surf",
        ],
        SpotlightCategory::Chat => &[
            "element-desktop",
            "signal-desktop",
            "fractal",
            "weechat",
            "discord",
        ],
        SpotlightCategory::Games => &["steam", "lutris", "minetest", "supertuxkart", "0ad"],
        SpotlightCategory::Email => &["thunderbird", "geary", "claws-mail", "mutt", "kmail"],
        SpotlightCategory::Productivity => &[
            "libreoffice",
            "onlyoffice-desktopeditors",
            "gnumeric",
            "abiword",
            "zim",
        ],
        SpotlightCategory::Utilities => &["htop", "ripgrep", "tmux", "neovim", "git"],
        SpotlightCategory::Graphics => &["gimp", "inkscape", "krita", "blender", "darktable"],
        SpotlightCategory::Music => &["audacity", "ardour", "lmms", "hydrogen", "mpd"],
        SpotlightCategory::Video => &["vlc", "mpv", "kdenlive", "obs-studio", "handbrake"],
    }
}

pub(crate) fn all_spotlight_categories() -> &'static [SpotlightCategory] {
    &[
        SpotlightCategory::Browsers,
        SpotlightCategory::Chat,
        SpotlightCategory::Email,
        SpotlightCategory::Games,
        SpotlightCategory::Graphics,
        SpotlightCategory::Music,
        SpotlightCategory::Productivity,
        SpotlightCategory::Utilities,
        SpotlightCategory::Video,
    ]
}

pub(crate) fn category_display_name(category: SpotlightCategory) -> &'static str {
    match category {
        SpotlightCategory::Browsers => "Browsers",
        SpotlightCategory::Chat => "Chat",
        SpotlightCategory::Email => "E-mail",
        SpotlightCategory::Games => "Games",
        SpotlightCategory::Graphics => "Graphics",
        SpotlightCategory::Music => "Music",
        SpotlightCategory::Productivity => "Productivity",
        SpotlightCategory::Utilities => "Utilities",
        SpotlightCategory::Video => "Video",
    }
}
