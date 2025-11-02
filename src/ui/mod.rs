pub(crate) mod app;
pub(crate) mod discover;
pub(crate) mod installed;
pub(crate) mod theme;
pub(crate) mod tools;
pub(crate) mod updates;

pub(crate) use app::{AppWidgets, build_ui};
pub(crate) use discover::{DiscoverWidgets, build_page as build_discover_page};
pub(crate) use installed::{InstalledWidgets, build_page as build_installed_page};
pub(crate) use theme::{ThemeGlyph, apply_theme_css_class, build_theme_icon};
pub(crate) use tools::{ToolsWidgets, build_page as build_tools_page};
pub(crate) use updates::{UpdatesWidgets, build_page as build_updates_page};
