use std::env;
use std::fs;
use std::path::PathBuf;

use libadwaita as adw;
use serde::{Deserialize, Serialize};

const APP_SETTINGS_FILE: &str = "settings.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartPagePreference {
    Discover,
    LastVisited,
}

impl Default for StartPagePreference {
    fn default() -> Self {
        StartPagePreference::Discover
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckFrequency {
    Daily,
    Weekly,
}

impl Default for UpdateCheckFrequency {
    fn default() -> Self {
        UpdateCheckFrequency::Daily
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl Default for ThemePreference {
    fn default() -> Self {
        ThemePreference::System
    }
}

impl ThemePreference {
    pub fn key(self) -> &'static str {
        match self {
            ThemePreference::System => "system",
            ThemePreference::Light => "light",
            ThemePreference::Dark => "dark",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => ThemePreference::System,
        }
    }

    pub fn apply(self, style_manager: &adw::StyleManager) {
        match self {
            ThemePreference::System => style_manager.set_color_scheme(adw::ColorScheme::Default),
            ThemePreference::Light => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
            ThemePreference::Dark => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub window_width: Option<i32>,
    #[serde(default)]
    pub window_height: Option<i32>,
    #[serde(default)]
    pub start_page: StartPagePreference,
    #[serde(default)]
    pub last_page: Option<String>,
    #[serde(default = "default_auto_check_enabled")]
    pub auto_check_enabled: bool,
    #[serde(default)]
    pub auto_check_frequency: UpdateCheckFrequency,
    #[serde(default = "default_confirm_pref")]
    pub confirm_install: bool,
    #[serde(default = "default_confirm_pref")]
    pub confirm_remove: bool,
    #[serde(default)]
    pub theme_preference: ThemePreference,
    #[serde(default = "default_notify_updates")]
    pub notify_updates: bool,
    #[serde(default)]
    pub mirror_selection: Vec<String>,
}

fn default_auto_check_enabled() -> bool {
    true
}

fn default_confirm_pref() -> bool {
    true
}

fn default_notify_updates() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            window_width: None,
            window_height: None,
            start_page: StartPagePreference::Discover,
            last_page: Some("discover".to_string()),
            auto_check_enabled: default_auto_check_enabled(),
            auto_check_frequency: UpdateCheckFrequency::Daily,
            confirm_install: default_confirm_pref(),
            confirm_remove: default_confirm_pref(),
            theme_preference: ThemePreference::System,
            notify_updates: default_notify_updates(),
            mirror_selection: Vec::new(),
        }
    }
}

pub fn load_app_settings() -> AppSettings {
    let Some(path) = app_settings_path() else {
        return AppSettings::default();
    };

    let Ok(content) = fs::read_to_string(&path) else {
        return AppSettings::default();
    };

    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save_app_settings(settings: &AppSettings) -> Result<(), String> {
    let Some(path) = app_settings_path() else {
        return Err("Unable to determine settings directory".to_string());
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create settings directory: {}", err))?;
    }

    let data = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("Failed to serialize settings: {}", err))?;

    fs::write(&path, data).map_err(|err| format!("Failed to write settings: {}", err))
}

fn app_config_dir() -> Option<PathBuf> {
    if let Ok(custom) = env::var("NEBULA_STORE_CONFIG_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    if let Ok(config_home) = env::var("XDG_CONFIG_HOME") {
        let trimmed = config_home.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("nebula-gtk"));
        }
    }

    if let Ok(home) = env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join(".config").join("nebula-gtk"));
        }
    }

    None
}

fn app_settings_path() -> Option<PathBuf> {
    app_config_dir().map(|dir| dir.join(APP_SETTINGS_FILE))
}
