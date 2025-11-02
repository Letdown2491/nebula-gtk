use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::thread;

use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use glib::{Variant, VariantTy};
use gtk::{gdk, gio, glib, pango};
use std::f64::consts::PI;

use chrono::{DateTime, Duration, FixedOffset, LocalResult, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
enum ThemeGlyph {
    System,
    Light,
    Dark,
}

const APP_ID: &str = "tech.geektoshi.Nebula";
const SPOTLIGHT_WINDOW_DAYS: i64 = 7;
const SPOTLIGHT_RECENT_LIMIT: usize = 25;
const SPOTLIGHT_CACHE_FILE: &str = "spotlight.json";
const SPOTLIGHT_CACHE_VERSION: u32 = 1;
const SPOTLIGHT_CACHE_MAX_ENTRIES: usize = 4096;
const SPOTLIGHT_REFRESH_INTERVAL_HOURS: i64 = 24;
const APP_SETTINGS_FILE: &str = "settings.json";

fn main() -> glib::ExitCode {
    adw::init().expect("Failed to initialize libadwaita");

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::FLAGS_NONE)
        .build();

    app.connect_activate(build_ui);

    app.run()
}

fn build_ui(app: &adw::Application) {
    gio::resources_register_include!("nebula.gresource")
        .expect("Failed to register embedded resources");

    let settings = Rc::new(RefCell::new(load_app_settings()));
    let (initial_width, initial_height) = {
        let settings = settings.borrow();
        (
            settings.window_width.unwrap_or(1100),
            settings.window_height.unwrap_or(720),
        )
    };

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Nebula")
        .default_width(initial_width)
        .default_height(initial_height)
        .build();
    window.add_css_class("nebula-window");

    let toast_overlay = adw::ToastOverlay::new();
    window.set_content(Some(&toast_overlay));

    let root_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root_box.add_css_class("nebula-root");
    toast_overlay.set_child(Some(&root_box));

    let view_stack = adw::ViewStack::new();

    let header_bar = adw::HeaderBar::new();
    header_bar.add_css_class("nebula-headerbar");
    header_bar.set_hexpand(true);
    let header_title = gtk::Label::builder()
        .label("Nebula")
        .halign(gtk::Align::Center)
        .build();
    header_title.add_css_class("nebula-header-title");
    header_bar.set_title_widget(Some(&header_title));
    header_bar.set_show_end_title_buttons(false);
    header_bar.set_show_start_title_buttons(false);
    root_box.append(&header_bar);

    let header_logo = gtk::Image::from_resource("/tech/geektoshi/Nebula/icons/nebula.png");
    header_logo.set_pixel_size(24);
    header_logo.set_margin_start(6);
    header_logo.set_valign(gtk::Align::Center);
    header_bar.pack_start(&header_logo);

    let style_manager = adw::StyleManager::default();
    let stored_theme = {
        let settings = settings.borrow();
        settings.theme_preference
    };
    stored_theme.apply(&style_manager);
    let current_theme = stored_theme.key().to_string();
    apply_theme_css_class(&window, style_manager.is_dark());
    style_manager.connect_dark_notify(glib::clone!(@weak window => move |manager| {
        apply_theme_css_class(&window, manager.is_dark());
    }));

    let theme_action = gio::SimpleAction::new_stateful(
        "theme",
        Some(&VariantTy::STRING),
        &Variant::from(current_theme.as_str()),
    );
    theme_action.connect_change_state(
        glib::clone!(@weak style_manager, @strong settings => move |action, value| {
            let Some(value) = value else {
                return;
            };
            if let Some(theme) = value.str() {
                action.set_state(value);
                let preference = ThemePreference::from_key(theme);
                preference.apply(&style_manager);
                {
                    let mut data = settings.borrow_mut();
                    data.theme_preference = preference;
                    if let Err(err) = save_app_settings(&data) {
                        eprintln!("Failed to save settings: {}", err);
                    }
                }
            }
        }),
    );
    app.add_action(&theme_action);

    let preferences_action = gio::SimpleAction::new("preferences", None);
    app.add_action(&preferences_action);

    let show_updates_action = gio::SimpleAction::new("show-updates", None);
    app.add_action(&show_updates_action);

    let about_action = gio::SimpleAction::new("about", None);
    about_action.connect_activate(glib::clone!(@weak window => move |_, _| {
        let dialog = gtk::Dialog::builder()
            .transient_for(&window)
            .modal(true)
            .title("About Nebula")
            .build();
        dialog.add_button("Close", gtk::ResponseType::Close);
        dialog.connect_response(|dialog, _| dialog.close());

        let content = dialog.content_area();
        content.set_spacing(12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(24);
        content.set_margin_end(24);

        let layout = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .halign(gtk::Align::Center)
            .build();

        let logo = gtk::Image::from_resource("/tech/geektoshi/Nebula/icons/nebula.png");
        logo.set_pixel_size(64);
        layout.append(&logo);

        let name_label = gtk::Label::builder()
            .label("Nebula")
            .halign(gtk::Align::Center)
            .build();
        name_label.add_css_class("title-3");
        layout.append(&name_label);

        let version_label = gtk::Label::builder()
            .label(&format!("Version {}", env!("CARGO_PKG_VERSION")))
            .halign(gtk::Align::Center)
            .build();
        layout.append(&version_label);

        let description = gtk::Label::builder()
            .label("A GTK frontend for Void Linux's XBPS software toolkit.")
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .justify(gtk::Justification::Center)
            .halign(gtk::Align::Center)
            .build();
        layout.append(&description);

        let link = gtk::LinkButton::builder()
            .label("https://github.com/Letdown2491/nebula-gtk")
            .uri("https://github.com/Letdown2491/nebula-gtk")
            .halign(gtk::Align::Center)
            .build();
        layout.append(&link);

        content.append(&layout);
        dialog.present();
    }));
    app.add_action(&about_action);

    let display = gdk::Display::default().expect("No display");
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_resource("/tech/geektoshi/Nebula/style.css");
    gtk::style_context_add_provider_for_display(
        &display,
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .has_frame(false)
        .build();
    menu_button.add_css_class("flat");
    menu_button.add_css_class("nebula-header-button");

    let popover = gtk::Popover::builder()
        .has_arrow(true)
        .autohide(true)
        .build();
    let popover_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();

    let mut theme_buttons: Vec<(String, gtk::Button)> = Vec::new();
    let mut build_theme_row = |key: &str, icon_mode: ThemeGlyph| {
        let button = gtk::Button::builder()
            .has_frame(false)
            .can_focus(false)
            .hexpand(false)
            .vexpand(false)
            .build();
        button.add_css_class("menuitem");
        button.add_css_class("theme-circle");
        button.add_css_class("flat");
        button.set_focus_on_click(false);
        button.set_width_request(44);
        button.set_height_request(44);

        let icon = build_theme_icon(icon_mode);
        icon.set_halign(gtk::Align::Center);
        icon.set_valign(gtk::Align::Center);
        button.set_child(Some(&icon));

        theme_buttons.push((key.to_string(), button.clone()));
        button
    };

    let system_row = build_theme_row("system", ThemeGlyph::System);
    let light_row = build_theme_row("light", ThemeGlyph::Light);
    let dark_row = build_theme_row("dark", ThemeGlyph::Dark);

    let theme_list = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(18)
        .halign(gtk::Align::Center)
        .build();
    theme_list.set_margin_top(4);
    theme_list.set_margin_bottom(4);
    theme_list.append(&system_row);
    theme_list.append(&light_row);
    theme_list.append(&dark_row);
    popover_box.append(&theme_list);

    let separator = gtk::Separator::builder()
        .orientation(gtk::Orientation::Horizontal)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    popover_box.append(&separator);

    let prefs_row = gtk::Button::builder()
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .has_frame(false)
        .build();
    prefs_row.add_css_class("menuitem");
    let prefs_label = gtk::Label::builder()
        .label("Preferences")
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .build();
    prefs_row.set_child(Some(&prefs_label));

    let about_row = gtk::Button::builder()
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .has_frame(false)
        .build();
    about_row.add_css_class("menuitem");
    let about_label = gtk::Label::builder()
        .label("About Nebula")
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .build();
    about_row.set_child(Some(&about_label));

    popover_box.append(&prefs_row);
    popover_box.append(&about_row);

    popover.set_child(Some(&popover_box));
    menu_button.set_popover(Some(&popover));

    let theme_buttons_rc = Rc::new(theme_buttons);
    let refresh_theme_buttons = |scheme: adw::ColorScheme, buttons: &[(String, gtk::Button)]| {
        let key = match scheme {
            adw::ColorScheme::ForceLight => "light",
            adw::ColorScheme::ForceDark => "dark",
            _ => "system",
        };
        for (name, button) in buttons.iter() {
            let ctx = button.style_context();
            if name == key {
                ctx.add_class("theme-active");
            } else {
                ctx.remove_class("theme-active");
            }
        }
    };
    refresh_theme_buttons(style_manager.color_scheme(), &theme_buttons_rc);
    style_manager.connect_color_scheme_notify(
        glib::clone!(@strong theme_buttons_rc => move |manager| {
            refresh_theme_buttons(manager.color_scheme(), &theme_buttons_rc);
        }),
    );

    system_row.connect_clicked(glib::clone!(@weak theme_action, @weak popover, @weak style_manager, @strong theme_buttons_rc => move |_| {
        let state = Variant::from("system");
        theme_action.change_state(&state);
        refresh_theme_buttons(style_manager.color_scheme(), &theme_buttons_rc);
        popover.popdown();
    }));
    light_row.connect_clicked(glib::clone!(@weak theme_action, @weak popover, @weak style_manager, @strong theme_buttons_rc => move |_| {
        let state = Variant::from("light");
        theme_action.change_state(&state);
        refresh_theme_buttons(style_manager.color_scheme(), &theme_buttons_rc);
        popover.popdown();
    }));
    dark_row.connect_clicked(glib::clone!(@weak theme_action, @weak popover, @weak style_manager, @strong theme_buttons_rc => move |_| {
        let state = Variant::from("dark");
        theme_action.change_state(&state);
        refresh_theme_buttons(style_manager.color_scheme(), &theme_buttons_rc);
        popover.popdown();
    }));

    prefs_row.connect_clicked(glib::clone!(@weak popover => move |_| {
        popover.popdown();
        gio::Application::default()
            .and_then(|app| app.lookup_action("preferences"))
            .map(|action| action.activate(None));
    }));
    about_row.connect_clicked(glib::clone!(@weak popover => move |_| {
        popover.popdown();
        gio::Application::default()
            .and_then(|app| app.lookup_action("about"))
            .map(|action| action.activate(None));
    }));

    let window_controls = gtk::WindowControls::new(gtk::PackType::End);
    let header_controls_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    header_controls_box.append(&menu_button);
    header_controls_box.append(&window_controls);
    header_bar.pack_end(&header_controls_box);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(0)
        .margin_bottom(0)
        .margin_start(0)
        .margin_end(0)
        .build();
    content.set_vexpand(true);
    content.set_hexpand(true);
    content.add_css_class("nebula-content");
    root_box.append(&content);

    let (discover_page, discover_widgets) = build_discover_page();
    let (installed_page, installed_widgets) = build_installed_page();
    let (updates_page, updates_widgets) = build_updates_page();

    let discover_page_ref = view_stack.add_titled(&discover_page, Some("discover"), "Discover");
    discover_page_ref.set_icon_name(Some(""));
    let installed_page_ref = view_stack.add_titled(&installed_page, Some("installed"), "Installed");
    installed_page_ref.set_icon_name(Some(""));
    let updates_page_ref = view_stack.add_titled(&updates_page, Some("updates"), "Updates");
    updates_page_ref.set_icon_name(Some(""));
    updates_page_ref.set_badge_number(0);
    view_stack.set_vexpand(true);

    let switcher_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    switcher_box.add_css_class("nebula-switcher");
    switcher_box.set_halign(gtk::Align::Center);
    switcher_box.set_margin_bottom(6);

    let discover_button = gtk::ToggleButton::builder().label("Discover").build();
    discover_button.add_css_class("flat");
    discover_button.set_active(true);

    let installed_button = gtk::ToggleButton::builder().label("Installed").build();
    installed_button.add_css_class("flat");
    installed_button.set_group(Some(&discover_button));

    let updates_button = gtk::ToggleButton::builder().build();
    updates_button.add_css_class("flat");
    updates_button.set_group(Some(&discover_button));

    let updates_content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(5)
        .halign(gtk::Align::Center)
        .build();
    let updates_label = gtk::Label::new(Some("Updates"));
    let updates_badge = gtk::Label::new(Some("0"));
    updates_badge.add_css_class("tag");
    updates_badge.add_css_class("accent");
    updates_badge.add_css_class("nebula-badge");
    updates_badge.set_xalign(0.5);
    updates_badge.set_yalign(0.5);
    updates_badge.set_halign(gtk::Align::Center);
    updates_badge.set_valign(gtk::Align::Center);
    updates_badge.set_margin_start(4);
    updates_badge.set_visible(false);
    updates_content.append(&updates_label);
    updates_content.append(&updates_badge);
    updates_button.set_child(Some(&updates_content));

    switcher_box.append(&discover_button);
    switcher_box.append(&installed_button);
    switcher_box.append(&updates_button);

    content.append(&switcher_box);
    content.append(&view_stack);

    let widgets = AppWidgets {
        toast_overlay: toast_overlay.clone(),
        view_stack: view_stack.clone(),
        discover: discover_widgets,
        installed: installed_widgets,
        updates: updates_widgets,
        updates_page: updates_page_ref,
        discover_button: discover_button.clone(),
        installed_button: installed_button.clone(),
        updates_button: updates_button.clone(),
        updates_badge: updates_badge.clone(),
    };

    let (sender, receiver) = mpsc::channel::<AppMessage>();
    let receiver = Rc::new(RefCell::new(receiver));
    let controller = Rc::new(AppController::new(
        widgets,
        sender,
        app.clone(),
        window.clone(),
        settings.clone(),
    ));

    let controller_clone = controller.clone();
    let receiver_clone = receiver.clone();
    glib::idle_add_local(move || {
        let receiver = receiver_clone.borrow_mut();
        while let Ok(msg) = receiver.try_recv() {
            controller_clone.handle_message(msg);
        }
        glib::ControlFlow::Continue
    });

    controller.setup_connections();
    controller.apply_start_page_preference();

    {
        let controller_weak = Rc::downgrade(&controller);
        show_updates_action.connect_activate(move |_, _| {
            if let Some(controller) = controller_weak.upgrade() {
                controller.set_active_page("updates");
                controller.window.present();
            }
        });
    }

    {
        let controller_weak = Rc::downgrade(&controller);
        preferences_action.connect_activate(move |_, _| {
            if let Some(controller) = controller_weak.upgrade() {
                controller.show_preferences();
            }
        });
    }
    controller.initialize_spotlight();
    controller.refresh_installed_packages();
    controller.refresh_updates(true);

    let settings_for_close = Rc::clone(&settings);
    window.connect_close_request(move |win| {
        let width = win.width();
        let height = win.height();
        if width > 0 && height > 0 {
            {
                let mut data = settings_for_close.borrow_mut();
                data.window_width = Some(width);
                data.window_height = Some(height);
            }
            if let Err(err) = save_app_settings(&settings_for_close.borrow()) {
                eprintln!("Failed to save settings: {}", err);
            }
        }
        glib::Propagation::Proceed
    });

    window.present();
}

fn apply_theme_css_class(window: &adw::ApplicationWindow, is_dark: bool) {
    if is_dark {
        window.remove_css_class("nebula-theme-light");
        window.add_css_class("nebula-theme-dark");
    } else {
        window.remove_css_class("nebula-theme-dark");
        window.add_css_class("nebula-theme-light");
    }
}

fn build_theme_icon(mode: ThemeGlyph) -> gtk::DrawingArea {
    use gtk::prelude::*;

    let area = gtk::DrawingArea::builder()
        .content_width(32)
        .content_height(32)
        .build();

    area.set_draw_func(move |_area, cr, width, height| {
        let radius = (width.min(height) as f64) / 2.0 - 1.0;
        let cx = width as f64 / 2.0;
        let cy = height as f64 / 2.0;

        match mode {
            ThemeGlyph::Light => {
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
                let _ = cr.fill();
            }
            ThemeGlyph::Dark => {
                cr.set_source_rgb(0.08, 0.08, 0.08);
                cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
                let _ = cr.fill();
            }
            ThemeGlyph::System => {
                cr.set_source_rgb(0.08, 0.08, 0.08);
                cr.arc(cx, cy, radius, PI / 2.0, 3.0 * PI / 2.0);
                cr.line_to(cx, cy);
                cr.close_path();
                let _ = cr.fill();

                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.arc(cx, cy, radius, 3.0 * PI / 2.0, PI / 2.0);
                cr.line_to(cx, cy);
                cr.close_path();
                let _ = cr.fill();
            }
        }

        cr.set_source_rgb(0.0, 0.0, 0.0);
        cr.set_line_width(1.0);
        cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
        let _ = cr.stroke();
    });

    area
}

#[derive(Clone, Debug)]
struct PackageInfo {
    name: String,
    version: String,
    description: String,
    installed: bool,
    previous_version: Option<String>,
    download_size: Option<String>,
    changelog: Option<String>,
    download_bytes: Option<u64>,
    repository: Option<String>,
    build_date: Option<DateTime<Utc>>,
    first_seen: Option<DateTime<Utc>>,
    name_lower: Arc<str>,
    version_lower: Arc<str>,
    description_lower: Arc<str>,
}

fn lowercase_cache(value: &str) -> Arc<str> {
    if value.is_empty() {
        Arc::<str>::from("")
    } else {
        Arc::<str>::from(value.to_lowercase())
    }
}

impl PackageInfo {
    fn set_version(&mut self, version: String) {
        self.version = version;
        self.version_lower = lowercase_cache(&self.version);
    }

    fn set_description(&mut self, description: String) {
        self.description = description;
        self.description_lower = lowercase_cache(&self.description);
    }
}

#[derive(Debug)]
struct CommandResult {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl CommandResult {
    fn success(&self) -> bool {
        self.code.unwrap_or(-1) == 0
    }
}

#[derive(Default)]
struct AppState {
    search_results: Vec<PackageInfo>,
    installed_packages: Vec<PackageInfo>,
    installed_set: HashSet<String>,
    installed_filter: String,
    installed_filtered: Vec<usize>,
    installed_selected: HashSet<String>,
    installed_filter_mode: InstalledFilter,
    installed_last_refresh: Option<glib::DateTime>,
    selected_installed: Option<usize>,
    installed_detail_cache: HashMap<String, InstalledDetail>,
    installed_detail_loading: HashSet<String>,
    installed_detail_errors: HashMap<String, String>,
    installed_detail_package: Option<String>,
    installed_detail_history: Vec<String>,
    installed_detail_navigation_active: bool,
    installed_status_message: Option<String>,
    available_updates: Vec<PackageInfo>,
    available_update_names: HashSet<String>,
    updates_loading: bool,
    update_in_progress: bool,
    selected_updates: HashSet<String>,
    selected_update: Option<usize>,
    total_update_size: u64,
    last_update_check: Option<glib::DateTime>,
    auto_check_enabled: bool,
    auto_check_frequency: UpdateCheckFrequency,
    auto_check_source: Option<glib::SourceId>,
    selected_search: Option<usize>,
    search_in_progress: bool,
    install_in_progress: bool,
    remove_in_progress: bool,
    installed_refresh_in_progress: bool,
    spotlight_cache: SpotlightCache,
    spotlight_recent: Vec<PackageInfo>,
    spotlight_categories: HashMap<SpotlightCategory, Vec<PackageInfo>>,
    spotlight_loading: bool,
    spotlight_last_refresh: Option<DateTime<Utc>>,
    active_spotlight_category: Option<SpotlightCategory>,
    spotlight_search_backup: Option<Vec<PackageInfo>>,
    spotlight_status_backup: Option<String>,
    spotlight_recent_selected: Option<String>,
    discover_mode: DiscoverMode,
    discover_detail_cache: HashMap<String, DiscoverDetail>,
    discover_detail_loading: HashSet<String>,
    discover_detail_errors: HashMap<String, String>,
    discover_detail_history: Vec<String>,
    discover_detail_navigation_active: bool,
    discover_detail_package: Option<String>,
    pending_discover_target: Option<String>,
    discover_detail_focus: Option<PackageInfo>,
    updates_detail_package: Option<String>,
    updates_detail_cache: HashMap<String, InstalledDetail>,
    updates_detail_loading: HashSet<String>,
    updates_detail_errors: HashMap<String, String>,
    start_page_preference: StartPagePreference,
    confirm_install: bool,
    confirm_remove: bool,
    footer_message: Option<String>,
    notify_updates: bool,
    updates_notification_sent: bool,
}

struct AppWidgets {
    toast_overlay: adw::ToastOverlay,
    view_stack: adw::ViewStack,
    discover: DiscoverWidgets,
    installed: InstalledWidgets,
    updates: UpdatesWidgets,
    updates_page: adw::ViewStackPage,
    discover_button: gtk::ToggleButton,
    installed_button: gtk::ToggleButton,
    updates_button: gtk::ToggleButton,
    updates_badge: gtk::Label,
}

struct DiscoverWidgets {
    search_entry: gtk::SearchEntry,
    search_button: gtk::Button,
    search_spinner: gtk::Spinner,
    status_label: gtk::Label,
    list: gtk::ListBox,
    scroller: gtk::ScrolledWindow,
    content_row: gtk::Box,
    detail_stack: gtk::Stack,
    detail_name: gtk::Label,
    detail_back_button: gtk::Button,
    detail_close_button: gtk::Button,
    detail_version_value: gtk::Label,
    detail_repository_row: gtk::Box,
    detail_repository_value: gtk::Label,
    detail_description: gtk::Label,
    detail_download_value: gtk::Label,
    detail_homepage_row: gtk::Box,
    detail_homepage_link: gtk::LinkButton,
    detail_maintainer_row: gtk::Box,
    detail_maintainer_value: gtk::Label,
    detail_license_row: gtk::Box,
    detail_license_value: gtk::Label,
    detail_update_label: gtk::Label,
    detail_action_button: gtk::Button,
    detail_dependencies_stack: gtk::Stack,
    detail_dependencies_list: gtk::ListBox,
    detail_dependencies_placeholder: gtk::Label,
    detail_frame: gtk::Frame,
    spotlight_spinner: gtk::Spinner,
    spotlight_status: gtk::Label,
    spotlight_recent_stack: gtk::Stack,
    spotlight_recent_list: gtk::ListBox,
    spotlight_recent_scroller: gtk::ScrolledWindow,
    spotlight_recent_detail_revealer: gtk::Revealer,
    spotlight_recent_detail_container: gtk::Box,
    spotlight_recent_back_button: gtk::Button,
    spotlight_recent_detail_name: gtk::Label,
    spotlight_recent_detail_spinner: gtk::Spinner,
    spotlight_recent_detail_version_value: gtk::Label,
    spotlight_recent_detail_repo_row: gtk::Box,
    spotlight_recent_detail_repo_value: gtk::Label,
    spotlight_recent_detail_download_value: gtk::Label,
    spotlight_recent_detail_updated_row: gtk::Box,
    spotlight_recent_detail_updated_value: gtk::Label,
    spotlight_recent_detail_homepage_row: gtk::Box,
    spotlight_recent_detail_homepage_link: gtk::LinkButton,
    spotlight_recent_detail_maintainer_row: gtk::Box,
    spotlight_recent_detail_maintainer_value: gtk::Label,
    spotlight_recent_detail_license_row: gtk::Box,
    spotlight_recent_detail_license_value: gtk::Label,
    spotlight_recent_detail_status: gtk::Label,
    spotlight_recent_detail_description: gtk::Label,
    spotlight_recent_detail_update_label: gtk::Label,
    spotlight_recent_detail_dependencies_stack: gtk::Stack,
    spotlight_recent_detail_dependencies_list: gtk::ListBox,
    spotlight_recent_detail_dependencies_placeholder: gtk::Label,
    spotlight_recent_action_button: gtk::Button,
    spotlight_section_box: gtk::Box,
    category_browsers_button: gtk::ToggleButton,
    category_chat_button: gtk::ToggleButton,
    category_email_button: gtk::ToggleButton,
    category_games_button: gtk::ToggleButton,
    category_graphics_button: gtk::ToggleButton,
    category_music_button: gtk::ToggleButton,
    category_productivity_button: gtk::ToggleButton,
    category_utilities_button: gtk::ToggleButton,
    category_video_button: gtk::ToggleButton,
    spotlight_refresh_button: gtk::Button,
}

struct InstalledWidgets {
    refresh_button: gtk::Button,
    search_entry: gtk::SearchEntry,
    status_label: gtk::Label,
    spinner: gtk::Spinner,
    filter_dropdown: gtk::DropDown,
    remove_selected_button: gtk::Button,
    list: gtk::ListBox,
    detail_stack: gtk::Stack,
    detail_frame: gtk::Frame,
    detail_remove_button: gtk::Button,
    detail_update_button: gtk::Button,
    detail_back_button: gtk::Button,
    detail_close_button: gtk::Button,
    detail_name: gtk::Label,
    detail_version_value: gtk::Label,
    detail_description: gtk::Label,
    detail_download_value: gtk::Label,
    detail_homepage_row: gtk::Box,
    detail_homepage_link: gtk::LinkButton,
    detail_maintainer_row: gtk::Box,
    detail_maintainer_value: gtk::Label,
    detail_license_row: gtk::Box,
    detail_license_value: gtk::Label,
    detail_required_by_stack: gtk::Stack,
    detail_required_by_list: gtk::ListBox,
    detail_required_by_placeholder: gtk::Label,
    detail_update_label: gtk::Label,
    footer_label: gtk::Label,
}

struct UpdatesWidgets {
    summary_row: gtk::Box,
    status_label: gtk::Label,
    list: gtk::ListBox,
    scroller: gtk::ScrolledWindow,
    content_row: gtk::Box,
    placeholder: gtk::Box,
    placeholder_label: gtk::Label,
    check_button: gtk::Button,
    refresh_button: gtk::Button,
    update_all_button: gtk::Button,
    spinner: gtk::Spinner,
    summary_label: gtk::Label,
    footer_label: gtk::Label,
    detail_frame: gtk::Frame,
    detail_stack: gtk::Stack,
    detail_name: gtk::Label,
    detail_close_button: gtk::Button,
    detail_version_value: gtk::Label,
    detail_download_value: gtk::Label,
    detail_homepage_row: gtk::Box,
    detail_homepage_link: gtk::LinkButton,
    detail_maintainer_row: gtk::Box,
    detail_maintainer_value: gtk::Label,
    detail_license_row: gtk::Box,
    detail_license_value: gtk::Label,
    detail_description: gtk::Label,
    detail_update_label: gtk::Label,
    detail_required_by_stack: gtk::Stack,
    detail_required_by_list: gtk::ListBox,
    detail_required_by_placeholder: gtk::Label,
    detail_update_button: gtk::Button,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum SpotlightCategory {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
enum DiscoverMode {
    #[default]
    Spotlight,
    Search,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StartPagePreference {
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
enum UpdateCheckFrequency {
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
enum ThemePreference {
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
    fn key(self) -> &'static str {
        match self {
            ThemePreference::System => "system",
            ThemePreference::Light => "light",
            ThemePreference::Dark => "dark",
        }
    }

    fn from_key(value: &str) -> Self {
        match value {
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => ThemePreference::System,
        }
    }

    fn apply(self, style_manager: &adw::StyleManager) {
        match self {
            ThemePreference::System => style_manager.set_color_scheme(adw::ColorScheme::Default),
            ThemePreference::Light => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
            ThemePreference::Dark => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AppSettings {
    #[serde(default)]
    window_width: Option<i32>,
    #[serde(default)]
    window_height: Option<i32>,
    #[serde(default)]
    start_page: StartPagePreference,
    #[serde(default)]
    last_page: Option<String>,
    #[serde(default = "default_auto_check_enabled")]
    auto_check_enabled: bool,
    #[serde(default)]
    auto_check_frequency: UpdateCheckFrequency,
    #[serde(default = "default_confirm_pref")]
    confirm_install: bool,
    #[serde(default = "default_confirm_pref")]
    confirm_remove: bool,
    #[serde(default)]
    theme_preference: ThemePreference,
    #[serde(default = "default_notify_updates")]
    notify_updates: bool,
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
        }
    }
}

#[derive(Clone, Debug, Default)]
struct SpotlightCache {
    generated_at: Option<DateTime<Utc>>,
    packages: HashMap<String, PackageInfo>,
}

struct SpotlightRefreshOutcome {
    cache: SpotlightCache,
    recent: Vec<PackageInfo>,
    categories: HashMap<SpotlightCategory, Vec<PackageInfo>>,
    refreshed_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct SpotlightCacheFile {
    version: u32,
    generated_at: Option<String>,
    packages: Vec<SpotlightCacheEntryData>,
}

#[derive(Serialize, Deserialize)]
struct SpotlightCacheEntryData {
    name: String,
    version: String,
    description: String,
    repository: Option<String>,
    build_date: Option<String>,
    first_seen: Option<String>,
}

struct RemotePackageMetadata {
    name: String,
    version: String,
    description: String,
    repository: Option<String>,
    build_date: Option<DateTime<Utc>>,
}

fn category_allowlist(category: SpotlightCategory) -> &'static [&'static str] {
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

fn all_spotlight_categories() -> &'static [SpotlightCategory] {
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

fn category_display_name(category: SpotlightCategory) -> &'static str {
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

fn format_relative_time(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let delta = now.signed_duration_since(timestamp);

    if delta.num_seconds() <= 0 {
        return "just now".to_string();
    }

    if delta.num_minutes() < 1 {
        return "just now".to_string();
    }

    if delta.num_hours() < 1 {
        let minutes = delta.num_minutes();
        return format!(
            "{} minute{} ago",
            minutes,
            if minutes == 1 { "" } else { "s" }
        );
    }

    if delta.num_hours() < 24 {
        let hours = delta.num_hours();
        return format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" });
    }

    let days = delta.num_days();
    if days < 7 {
        return format!("{} day{} ago", days, if days == 1 { "" } else { "s" });
    }

    timestamp.format("%Y-%m-%d %H:%M UTC").to_string()
}

fn parse_cached_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn format_cached_datetime(value: &DateTime<Utc>) -> String {
    value.to_rfc3339()
}

fn glib_datetime_to_chrono(dt: &glib::DateTime) -> Option<DateTime<Utc>> {
    let utc = dt.to_timezone(&glib::TimeZone::utc()).ok()?;
    let seconds = utc.to_unix();
    let micros = utc.microsecond() as i64;
    DateTime::<Utc>::from_timestamp_micros(seconds * 1_000_000 + micros)
}
fn parse_build_date_field(value: &str) -> Option<DateTime<Utc>> {
    let trimmed = value.trim().trim_matches(|c| c == '"' || c == '\'');
    if trimmed.is_empty() {
        return None;
    }

    if let Some((date_part, tz_name)) = trimmed.rsplit_once(' ') {
        if tz_name.chars().all(|c| c.is_ascii_alphabetic()) {
            if let Some(offset) = timezone_offset_from_abbreviation(tz_name) {
                if let Some(result) = parse_with_fixed_offset(date_part.trim(), offset) {
                    return Some(result);
                }
            }
        }
    }

    let mut iso_candidate = trimmed.replace(" UTC", "Z");
    if !iso_candidate.contains('T') {
        iso_candidate = iso_candidate.replace(' ', "T");
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(&iso_candidate) {
        return Some(parsed.with_timezone(&Utc));
    }

    if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(Utc.from_utc_datetime(&parsed));
    }

    if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M") {
        return Some(Utc.from_utc_datetime(&parsed));
    }

    None
}

fn parse_with_fixed_offset(date_part: &str, offset: FixedOffset) -> Option<DateTime<Utc>> {
    const FORMATS: [&str; 2] = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"];

    for format in FORMATS {
        if let Ok(naive) = NaiveDateTime::parse_from_str(date_part, format) {
            match offset.from_local_datetime(&naive) {
                LocalResult::Single(dt) => return Some(dt.with_timezone(&Utc)),
                LocalResult::Ambiguous(first, second) => {
                    return Some(first.max(second).with_timezone(&Utc));
                }
                LocalResult::None => continue,
            }
        }
    }

    None
}

fn timezone_offset_from_abbreviation(name: &str) -> Option<FixedOffset> {
    match name {
        "UTC" | "GMT" => FixedOffset::east_opt(0),
        "CET" => FixedOffset::east_opt(3600),
        "CEST" => FixedOffset::east_opt(7200),
        "EET" => FixedOffset::east_opt(7200),
        "EEST" => FixedOffset::east_opt(10800),
        "PST" => FixedOffset::west_opt(8 * 3600),
        "PDT" => FixedOffset::west_opt(7 * 3600),
        "MST" => FixedOffset::west_opt(7 * 3600),
        "MDT" => FixedOffset::west_opt(6 * 3600),
        "CST" => FixedOffset::west_opt(6 * 3600),
        "CDT" => FixedOffset::west_opt(5 * 3600),
        "EST" => FixedOffset::west_opt(5 * 3600),
        "EDT" => FixedOffset::west_opt(4 * 3600),
        "BST" => FixedOffset::east_opt(3600),
        "IST" => FixedOffset::east_opt(19800),
        "JST" => FixedOffset::east_opt(9 * 3600),
        _ => None,
    }
}

fn spotlight_cache_dir() -> Option<PathBuf> {
    if let Ok(custom) = env::var("NEBULA_STORE_CACHE_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    if let Ok(cache_home) = env::var("XDG_CACHE_HOME") {
        let trimmed = cache_home.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("nebula-gtk"));
        }
    }

    if let Ok(home) = env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join(".cache").join("nebula-gtk"));
        }
    }

    None
}

fn spotlight_cache_path() -> Option<PathBuf> {
    spotlight_cache_dir().map(|dir| dir.join(SPOTLIGHT_CACHE_FILE))
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

fn load_app_settings() -> AppSettings {
    let Some(path) = app_settings_path() else {
        return AppSettings::default();
    };

    let Ok(content) = fs::read_to_string(&path) else {
        return AppSettings::default();
    };

    serde_json::from_str(&content).unwrap_or_default()
}

fn save_app_settings(settings: &AppSettings) -> Result<(), String> {
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

fn load_spotlight_cache_from_disk() -> SpotlightCache {
    let Some(path) = spotlight_cache_path() else {
        return SpotlightCache::default();
    };

    let Ok(content) = fs::read_to_string(&path) else {
        return SpotlightCache::default();
    };

    let Ok(file) = serde_json::from_str::<SpotlightCacheFile>(&content) else {
        return SpotlightCache::default();
    };

    if file.version != SPOTLIGHT_CACHE_VERSION {
        return SpotlightCache::default();
    }

    let mut cache = SpotlightCache::default();
    cache.generated_at = file.generated_at.as_deref().and_then(parse_cached_datetime);

    for entry in file.packages {
        if entry.name.is_empty() {
            continue;
        }

        let build_date = entry.build_date.as_deref().and_then(parse_cached_datetime);
        let first_seen = entry.first_seen.as_deref().and_then(parse_cached_datetime);

        let name = entry.name;
        let version = entry.version;
        let description = entry.description;
        let repository = entry.repository;

        let info = PackageInfo {
            name_lower: lowercase_cache(&name),
            version_lower: lowercase_cache(&version),
            description_lower: lowercase_cache(&description),
            name,
            version,
            description,
            installed: false,
            previous_version: None,
            download_size: None,
            changelog: None,
            download_bytes: None,
            repository,
            build_date,
            first_seen,
        };

        cache.packages.insert(info.name.clone(), info);
    }

    prune_spotlight_cache(&mut cache);

    cache
}

fn save_spotlight_cache_to_disk(cache: &SpotlightCache) -> Result<(), String> {
    let Some(path) = spotlight_cache_path() else {
        return Err("Unable to determine spotlight cache directory".to_string());
    };

    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return Err(format!("Failed to create cache directory: {}", err));
        }
    }

    let packages: Vec<SpotlightCacheEntryData> = cache
        .packages
        .values()
        .map(|info| SpotlightCacheEntryData {
            name: info.name.clone(),
            version: info.version.clone(),
            description: info.description.clone(),
            repository: info.repository.clone(),
            build_date: info.build_date.as_ref().map(format_cached_datetime),
            first_seen: info.first_seen.as_ref().map(format_cached_datetime),
        })
        .collect();

    let file = SpotlightCacheFile {
        version: SPOTLIGHT_CACHE_VERSION,
        generated_at: cache.generated_at.as_ref().map(format_cached_datetime),
        packages,
    };

    let data = serde_json::to_string_pretty(&file)
        .map_err(|err| format!("Failed to serialize spotlight cache: {}", err))?;

    fs::write(&path, data).map_err(|err| format!("Failed to write spotlight cache: {}", err))
}

fn prune_spotlight_cache(cache: &mut SpotlightCache) {
    if cache.packages.len() <= SPOTLIGHT_CACHE_MAX_ENTRIES {
        return;
    }

    let mut entries: Vec<(String, Option<DateTime<Utc>>, Option<DateTime<Utc>>)> = cache
        .packages
        .iter()
        .map(|(name, info)| (name.clone(), info.build_date, info.first_seen))
        .collect();

    entries.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.0.cmp(&b.0))
    });

    for (name, _, _) in entries.into_iter().skip(SPOTLIGHT_CACHE_MAX_ENTRIES) {
        cache.packages.remove(&name);
    }
}

fn fetch_remote_spotlight_metadata() -> Result<Vec<RemotePackageMetadata>, String> {
    let listings = Command::new("xbps-query")
        .args(["-R", "--regex", "-s", "."])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !listings.status.success() {
        let stderr = String::from_utf8_lossy(&listings.stderr);
        return Err(stderr.trim().to_string());
    }

    let build_dates = Command::new("xbps-query")
        .args(["-R", "--regex", "-s", ".", "-p", "build-date"])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !build_dates.status.success() {
        let stderr = String::from_utf8_lossy(&build_dates.stderr);
        return Err(stderr.trim().to_string());
    }

    let mut records: HashMap<String, RemotePackageMetadata> = HashMap::new();

    for line in String::from_utf8_lossy(&listings.stdout).lines() {
        if let Some((name, version, description)) = parse_search_listing_line(line) {
            let entry = records
                .entry(name.clone())
                .or_insert_with(|| RemotePackageMetadata {
                    name: name.clone(),
                    version: version.clone(),
                    description: description.clone(),
                    repository: None,
                    build_date: None,
                });

            entry.version = version;
            if entry.description.is_empty() {
                entry.description = description;
            }
        }
    }

    for line in String::from_utf8_lossy(&build_dates.stdout).lines() {
        if let Some((name, version, build_date, repository)) = parse_build_date_listing_line(line) {
            let entry = records
                .entry(name.clone())
                .or_insert_with(|| RemotePackageMetadata {
                    name: name.clone(),
                    version: version.clone(),
                    description: String::new(),
                    repository: repository.clone(),
                    build_date,
                });

            entry.version = version;
            if entry.repository.is_none() {
                entry.repository = repository;
            }
            if build_date.is_some() {
                entry.build_date = build_date;
            }
        }
    }

    Ok(records.into_values().collect())
}

fn parse_search_listing_line(line: &str) -> Option<(String, String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.starts_with('[') {
        return None;
    }

    let payload = trimmed.get(3..)?.trim_start();
    let mut split_index = None;
    for (idx, ch) in payload.char_indices() {
        if ch.is_whitespace() {
            split_index = Some(idx);
            break;
        }
    }

    let idx = split_index?;
    let identifier = payload[..idx].trim();
    if identifier.is_empty() {
        return None;
    }
    let description = payload[idx..].trim().to_string();
    let (name, version) = split_package_identifier(identifier);

    Some((name, version, description))
}

fn parse_build_date_listing_line(
    line: &str,
) -> Option<(String, String, Option<DateTime<Utc>>, Option<String>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (identifier, rest) = trimmed.split_once(':')?;
    let identifier = identifier.trim();
    if identifier.is_empty() {
        return None;
    }

    let mut remainder = rest.trim();
    let mut repository = None;
    if let Some(open_paren) = remainder.rfind('(') {
        if remainder.ends_with(')') && open_paren < remainder.len() {
            let repo_candidate = &remainder[open_paren + 1..remainder.len() - 1].trim();
            if !repo_candidate.is_empty() {
                repository = Some(repo_candidate.to_string());
            }
            remainder = remainder[..open_paren].trim_end();
        }
    }

    let build_date = parse_build_date_field(remainder);
    let (name, version) = split_package_identifier(identifier);
    Some((name, version, build_date, repository))
}

fn build_category_results(cache: &SpotlightCache) -> HashMap<SpotlightCategory, Vec<PackageInfo>> {
    let mut results = HashMap::new();

    for category in all_spotlight_categories() {
        let mut packages = Vec::new();
        for name in category_allowlist(*category) {
            if let Some(info) = cache.packages.get(*name) {
                packages.push(info.clone());
            }
        }
        results.insert(*category, packages);
    }

    results
}

fn compute_spotlight_sections(cache: &SpotlightCache, now: DateTime<Utc>) -> Vec<PackageInfo> {
    let window_start = now - Duration::days(SPOTLIGHT_WINDOW_DAYS);

    let mut recent: Vec<PackageInfo> = cache
        .packages
        .values()
        .filter(|pkg| pkg.build_date.map_or(false, |dt| dt >= window_start))
        .cloned()
        .collect();

    recent.sort_by(|a, b| {
        b.build_date
            .cmp(&a.build_date)
            .then_with(|| b.first_seen.cmp(&a.first_seen))
            .then_with(|| a.name.cmp(&b.name))
    });

    if recent.is_empty() {
        recent = cache.packages.values().cloned().collect();
        recent.sort_by(|a, b| {
            b.build_date
                .cmp(&a.build_date)
                .then_with(|| b.first_seen.cmp(&a.first_seen))
                .then_with(|| a.name.cmp(&b.name))
        });
    }
    recent.truncate(SPOTLIGHT_RECENT_LIMIT);

    recent
}

fn refresh_spotlight_cache(mut cache: SpotlightCache) -> Result<SpotlightRefreshOutcome, String> {
    let now = Utc::now();
    let remote_packages = fetch_remote_spotlight_metadata()?;

    for remote in remote_packages {
        if remote.name.is_empty() {
            continue;
        }

        let RemotePackageMetadata {
            name,
            version,
            description,
            repository,
            build_date,
        } = remote;

        let build_date_for_entry = build_date.clone();

        let entry = cache
            .packages
            .entry(name.clone())
            .or_insert_with(|| PackageInfo {
                name_lower: lowercase_cache(&name),
                version_lower: lowercase_cache(&version),
                description_lower: lowercase_cache(&description),
                name: name.clone(),
                version: version.clone(),
                description: description.clone(),
                installed: false,
                previous_version: None,
                download_size: None,
                changelog: None,
                download_bytes: None,
                repository: repository.clone(),
                build_date: build_date_for_entry.clone(),
                first_seen: Some(now),
            });

        let version_changed = entry.version != version;
        if version_changed {
            entry.previous_version = Some(entry.version.clone());
        }

        entry.set_version(version.clone());
        entry.set_description(description.clone());
        entry.repository = repository.clone();

        if let Some(date) = build_date_for_entry.clone() {
            entry.build_date = Some(date);
        }

        if entry.first_seen.is_none() {
            entry.first_seen = Some(now);
        }
    }
    prune_spotlight_cache(&mut cache);
    cache.generated_at = Some(now);

    let categories = build_category_results(&cache);

    let mut recent = compute_spotlight_sections(&cache, now);
    recent.truncate(SPOTLIGHT_RECENT_LIMIT);

    #[cfg(debug_assertions)]
    {
        eprintln!(
            "Spotlight refresh fetched {} packages; recent={}",
            cache.packages.len(),
            recent.len(),
        );
    }

    Ok(SpotlightRefreshOutcome {
        cache,
        recent,
        categories,
        refreshed_at: now,
    })
}

#[derive(Clone, Debug)]
struct DependencyInfo {
    name: String,
}

#[derive(Clone, Debug, Default)]
struct InstalledDetail {
    download_bytes: Option<u64>,
    download_formatted: Option<String>,
    download_error: Option<String>,
    homepage: Option<String>,
    maintainer: Option<String>,
    license: Option<String>,
    dependencies: Vec<InstalledDependency>,
    dependencies_error: Option<String>,
    required_by: Vec<String>,
    required_by_error: Option<String>,
    long_description: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
struct InstalledDependency {
    name: String,
    installed: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default)]
struct DiscoverDetail {
    version: Option<String>,
    description: Option<String>,
    download: Option<String>,
    download_bytes: Option<u64>,
    homepage: Option<String>,
    maintainer: Option<String>,
    license: Option<String>,
    repository: Option<String>,
    dependencies: Vec<DependencyInfo>,
}

enum AppMessage {
    SearchFinished {
        query: String,
        result: Result<Vec<PackageInfo>, String>,
    },
    InstalledFinished {
        result: Result<Vec<PackageInfo>, String>,
    },
    InstallFinished {
        package: String,
        result: Result<CommandResult, String>,
    },
    RemoveFinished {
        package: String,
        result: Result<CommandResult, String>,
    },
    RemoveBatchFinished {
        packages: Vec<String>,
        result: Result<CommandResult, String>,
    },
    InstalledDetailsLoaded {
        package: String,
        result: Result<InstalledDetail, String>,
    },
    UpdatesDetailLoaded {
        package: String,
        result: Result<InstalledDetail, String>,
    },
    UpdatesRefreshed {
        packages: Vec<PackageInfo>,
        success: bool,
        error: Option<String>,
    },
    UpdateFinished {
        packages: Vec<String>,
        result: Result<CommandResult, String>,
        all: bool,
    },
    DiscoverDetailLoaded {
        package: String,
        result: Result<DiscoverDetail, String>,
    },
    SpotlightLoaded {
        recent: Vec<PackageInfo>,
        categories: HashMap<SpotlightCategory, Vec<PackageInfo>>,
        cache: SpotlightCache,
        refreshed_at: DateTime<Utc>,
    },
    SpotlightFailed {
        error: String,
    },
}

#[derive(Clone, Copy)]
enum RemoveOrigin {
    Discover,
    Installed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
enum InstalledFilter {
    #[default]
    All,
    Updates,
}

struct AppController {
    widgets: AppWidgets,
    state: RefCell<AppState>,
    sender: mpsc::Sender<AppMessage>,
    app: adw::Application,
    window: adw::ApplicationWindow,
    settings: Rc<RefCell<AppSettings>>,
    update_buttons: RefCell<Vec<gtk::Button>>,
    installed_buttons: RefCell<Vec<gtk::Button>>,
    discover_buttons: RefCell<Vec<gtk::Button>>,
}

impl AppController {
    fn new(
        widgets: AppWidgets,
        sender: mpsc::Sender<AppMessage>,
        app: adw::Application,
        window: adw::ApplicationWindow,
        settings: Rc<RefCell<AppSettings>>,
    ) -> Self {
        let mut state = AppState::default();
        let cache = load_spotlight_cache_from_disk();
        let now = Utc::now();
        let recent = compute_spotlight_sections(&cache, now);
        let categories = build_category_results(&cache);

        state.spotlight_cache = cache;
        state.spotlight_recent = recent;
        state.spotlight_categories = categories;
        state.spotlight_last_refresh = state.spotlight_cache.generated_at;
        {
            let settings_ref = settings.borrow();
            state.auto_check_enabled = settings_ref.auto_check_enabled;
            state.auto_check_frequency = settings_ref.auto_check_frequency;
            state.confirm_install = settings_ref.confirm_install;
            state.confirm_remove = settings_ref.confirm_remove;
            state.start_page_preference = settings_ref.start_page;
            state.notify_updates = settings_ref.notify_updates;
        }

        Self {
            widgets,
            sender,
            app,
            state: RefCell::new(state),
            window,
            settings,
            update_buttons: RefCell::new(Vec::new()),
            installed_buttons: RefCell::new(Vec::new()),
            discover_buttons: RefCell::new(Vec::new()),
        }
    }

    fn setup_connections(self: &Rc<Self>) {
        self.widgets.discover.search_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_search_requested();
            }),
        );

        self.widgets.discover.search_entry.connect_search_changed(
            glib::clone!(@strong self as controller => move |entry| {
                controller.on_discover_search_changed(entry.text().to_string());
            }),
        );

        self.widgets.discover.search_entry.connect_activate(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_search_requested();
            }),
        );

        self.widgets.discover.list.connect_row_selected(
            glib::clone!(@strong self as controller => move |_, row| {
                controller.on_search_row_selected(row.cloned());
            }),
        );
        self.widgets.discover.list.connect_row_activated(
            glib::clone!(@strong self as controller => move |_, _| {
                controller.on_discover_primary_action();
            }),
        );
        self.widgets.discover.detail_action_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_discover_primary_action();
            }),
        );
        self.widgets.discover.detail_back_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_discover_detail_back();
            }),
        );
        self.widgets.discover.detail_close_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_discover_detail_close();
            }),
        );
        self.widgets
            .discover
            .spotlight_recent_list
            .connect_row_selected(glib::clone!(@strong self as controller => move |_, row| {
                controller.on_spotlight_recent_selected(row.cloned());
            }));
        self.widgets
            .discover
            .spotlight_recent_list
            .connect_row_activated(glib::clone!(@strong self as controller => move |_, row| {
                controller.on_spotlight_row_activated(row);
            }));
        self.widgets
            .discover
            .spotlight_recent_back_button
            .connect_clicked(glib::clone!(@strong self as controller => move |_| {
                controller.on_spotlight_recent_back();
            }));
        self.widgets
            .discover
            .spotlight_recent_action_button
            .connect_clicked(glib::clone!(@strong self as controller => move |_| {
                controller.on_discover_primary_action();
            }));
        self.widgets
            .discover
            .category_browsers_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Browsers,
                    btn.is_active(),
                );
            }));
        self.widgets
            .discover
            .category_chat_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(SpotlightCategory::Chat, btn.is_active());
            }));
        self.widgets
            .discover
            .category_games_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(SpotlightCategory::Games, btn.is_active());
            }));
        self.widgets
            .discover
            .category_graphics_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Graphics,
                    btn.is_active(),
                );
            }));
        self.widgets.discover.category_email_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Email,
                    btn.is_active(),
                );
            }),
        );
        self.widgets.discover.category_music_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Music,
                    btn.is_active(),
                );
            }),
        );
        self.widgets
            .discover
            .category_productivity_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Productivity,
                    btn.is_active(),
                );
            }));
        self.widgets
            .discover
            .category_utilities_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Utilities,
                    btn.is_active(),
                );
            }));
        self.widgets
            .discover
            .category_video_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(SpotlightCategory::Video, btn.is_active());
            }));

        self.widgets
            .discover
            .spotlight_refresh_button
            .connect_clicked(glib::clone!(@strong self as controller => move |_| {
                controller.maybe_refresh_spotlight(true);
            }));

        self.widgets.installed.refresh_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.refresh_installed_packages();
            }),
        );

        self.widgets.installed.search_entry.connect_search_changed(
            glib::clone!(@strong self as controller => move |entry| {
                controller.on_installed_search_changed(entry.text().to_string());
            }),
        );

        self.widgets.installed.search_entry.connect_activate(
            glib::clone!(@strong self as controller => move |entry| {
                controller.on_installed_search_changed(entry.text().to_string());
            }),
        );

        self.widgets.discover_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                if btn.is_active() {
                    controller.switch_to_page("discover");
                }
            }),
        );

        self.widgets.installed_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                if btn.is_active() {
                    controller.switch_to_page("installed");
                }
            }),
        );

        self.widgets.updates_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                if btn.is_active() {
                    controller.switch_to_page("updates");
                }
            }),
        );

        self.widgets
            .installed
            .filter_dropdown
            .connect_selected_notify(glib::clone!(@strong self as controller => move |dropdown| {
                controller.on_installed_filter_changed(dropdown.selected());
            }));

        self.widgets.installed.list.connect_row_selected(
            glib::clone!(@strong self as controller => move |_, row| {
                controller.on_installed_row_selected(row.cloned());
            }),
        );

        self.widgets
            .installed
            .remove_selected_button
            .connect_clicked(glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_remove_selected();
            }));

        self.widgets.installed.detail_back_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_detail_back();
            }),
        );
        self.widgets.installed.detail_close_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_detail_close();
            }),
        );

        self.widgets.installed.detail_remove_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_detail_remove();
            }),
        );

        self.widgets.installed.detail_update_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_detail_update();
            }),
        );

        {
            let state = self.state.borrow();
            let filter_index = match state.installed_filter_mode {
                InstalledFilter::All => 0,
                InstalledFilter::Updates => 1,
            };
            self.widgets
                .installed
                .filter_dropdown
                .set_selected(filter_index);
        }

        self.update_installed_summary();
        self.update_installed_selection_ui();
        self.update_installed_details();

        self.widgets.updates.check_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.refresh_updates(false);
            }),
        );
        self.widgets.updates.refresh_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.refresh_updates(false);
            }),
        );

        self.widgets.updates.update_all_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.update_all_packages();
            }),
        );

        self.widgets.updates.list.connect_row_activated(
            glib::clone!(@strong self as controller => move |_, row| {
                controller.on_update_row_activated(row);
            }),
        );
        self.widgets.updates.detail_close_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_updates_detail_close();
            }),
        );
        self.widgets.updates.detail_update_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_updates_detail_update();
            }),
        );

        self.widgets.updates.list.connect_row_selected(
            glib::clone!(@strong self as controller => move |_, row| {
                controller.on_update_row_selected(row.cloned());
            }),
        );

        let auto_enabled = self.state.borrow().auto_check_enabled;
        if auto_enabled {
            self.schedule_auto_check();
        }

        let weak_self = Rc::downgrade(self);
        self.widgets
            .view_stack
            .connect_visible_child_name_notify(move |_| {
                if let Some(controller) = weak_self.upgrade() {
                    controller.on_view_changed();
                }
            });

        self.update_footer_text();
        self.update_updates_badge();
        self.on_view_changed();
    }

    fn persist_settings(&self) {
        if let Err(err) = save_app_settings(&self.settings.borrow()) {
            eprintln!("Failed to save settings: {}", err);
        }
    }

    fn apply_start_page_preference(&self) {
        let (preference, last_page) = {
            let settings = self.settings.borrow();
            (
                settings.start_page,
                settings
                    .last_page
                    .clone()
                    .unwrap_or_else(|| "discover".to_string()),
            )
        };
        {
            let mut state = self.state.borrow_mut();
            state.start_page_preference = preference;
        }
        self.set_active_page(match preference {
            StartPagePreference::Discover => "discover",
            StartPagePreference::LastVisited => last_page.as_str(),
        });
    }

    fn set_active_page(&self, page: &str) {
        match page {
            "installed" => {
                if !self.widgets.installed_button.is_active() {
                    self.widgets.installed_button.set_active(true);
                } else {
                    self.switch_to_page("installed");
                }
            }
            "updates" => {
                if !self.widgets.updates_button.is_active() {
                    self.widgets.updates_button.set_active(true);
                } else {
                    self.switch_to_page("updates");
                }
            }
            _ => {
                if !self.widgets.discover_button.is_active() {
                    self.widgets.discover_button.set_active(true);
                } else {
                    self.switch_to_page("discover");
                }
            }
        }
    }

    fn switch_to_page(&self, page: &str) {
        if self.widgets.view_stack.visible_child_name().as_deref() != Some(page) {
            self.widgets.view_stack.set_visible_child_name(page);
        }
        self.record_last_page(page);
    }

    fn record_last_page(&self, page: &str) {
        {
            let mut settings = self.settings.borrow_mut();
            settings.last_page = Some(page.to_string());
        }
        self.persist_settings();
    }

    fn update_start_page_preference(&self, preference: StartPagePreference) {
        {
            let mut state = self.state.borrow_mut();
            state.start_page_preference = preference;
        }
        {
            let mut settings = self.settings.borrow_mut();
            settings.start_page = preference;
        }
        self.persist_settings();
    }

    fn set_auto_check_enabled(self: &Rc<Self>, enabled: bool, persist: bool) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.auto_check_enabled == enabled {
                false
            } else {
                state.auto_check_enabled = enabled;
                true
            }
        };

        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.auto_check_enabled = enabled;
            }
            self.persist_settings();
        }

        if enabled {
            self.schedule_auto_check();
        } else if changed {
            self.cancel_auto_check_timer();
        }
    }

    fn set_auto_check_frequency(self: &Rc<Self>, frequency: UpdateCheckFrequency, persist: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.auto_check_frequency = frequency;
        }

        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.auto_check_frequency = frequency;
            }
            self.persist_settings();
        }

        if self.state.borrow().auto_check_enabled {
            self.schedule_auto_check();
        }
    }

    fn set_confirm_install(&self, enabled: bool, persist: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.confirm_install = enabled;
        }
        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.confirm_install = enabled;
            }
            self.persist_settings();
        }
    }

    fn set_confirm_remove(&self, enabled: bool, persist: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.confirm_remove = enabled;
        }
        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.confirm_remove = enabled;
            }
            self.persist_settings();
        }
    }

    fn set_notify_updates(self: &Rc<Self>, enabled: bool, persist: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.notify_updates = enabled;
            state.updates_notification_sent = false;
        }
        self.withdraw_updates_notification();
        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.notify_updates = enabled;
            }
            self.persist_settings();
        }
    }

    fn confirm_action<F>(
        self: &Rc<Self>,
        heading: &str,
        body: &str,
        confirm_label: &str,
        on_confirm: F,
    ) where
        F: FnOnce(&Rc<Self>) + 'static,
    {
        let dialog = gtk::MessageDialog::builder()
            .text(heading)
            .secondary_text(body)
            .message_type(gtk::MessageType::Question)
            .modal(true)
            .build();
        dialog.set_transient_for(Some(&self.window));
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button(confirm_label, gtk::ResponseType::Accept);
        dialog.set_default_response(gtk::ResponseType::Accept);
        let controller_weak = Rc::downgrade(self);
        let callback = Rc::new(RefCell::new(Some(on_confirm)));
        dialog.connect_response(move |dlg, response| {
            dlg.close();
            if response == gtk::ResponseType::Accept {
                if let Some(controller) = controller_weak.upgrade() {
                    if let Some(callback) = callback.borrow_mut().take() {
                        callback(&controller);
                    }
                }
            }
        });
        dialog.present();
    }

    fn show_preferences(self: &Rc<Self>) {
        let prefs = adw::PreferencesWindow::builder()
            .transient_for(&self.window)
            .modal(true)
            .title("Preferences")
            .build();

        let general_page = adw::PreferencesPage::builder().title("General").build();

        let startup_group = adw::PreferencesGroup::builder()
            .title("Startup")
            .description("Choose what Nebula shows when it launches.")
            .build();
        let startup_model = gtk::StringList::new(&["Discover page", "Last viewed page"]);
        let start_combo = adw::ComboRow::builder()
            .title("Startup page")
            .model(&startup_model)
            .selected(match self.state.borrow().start_page_preference {
                StartPagePreference::LastVisited => 1,
                StartPagePreference::Discover => 0,
            })
            .build();
        startup_group.add(&start_combo);
        general_page.add(&startup_group);

        let updates_group = adw::PreferencesGroup::builder()
            .title("Updates")
            .description("Control automatic update checks.")
            .build();
        let auto_switch_row = adw::ActionRow::builder()
            .title("Check for updates automatically")
            .build();
        let auto_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        auto_switch.set_active(self.state.borrow().auto_check_enabled);
        auto_switch_row.add_suffix(&auto_switch);
        auto_switch_row.set_activatable_widget(Some(&auto_switch));

        let frequency_model = gtk::StringList::new(&["Daily", "Weekly"]);
        let freq_combo = adw::ComboRow::builder()
            .title("Frequency")
            .model(&frequency_model)
            .selected(match self.state.borrow().auto_check_frequency {
                UpdateCheckFrequency::Daily => 0,
                UpdateCheckFrequency::Weekly => 1,
            })
            .build();
        freq_combo.set_sensitive(self.state.borrow().auto_check_enabled);

        updates_group.add(&auto_switch_row);
        updates_group.add(&freq_combo);

        let notify_switch_row = adw::ActionRow::builder()
            .title("Show system notifications for new updates")
            .build();
        let notify_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        notify_switch.set_active(self.state.borrow().notify_updates);
        notify_switch_row.add_suffix(&notify_switch);
        notify_switch_row.set_activatable_widget(Some(&notify_switch));
        updates_group.add(&notify_switch_row);
        general_page.add(&updates_group);

        let install_group = adw::PreferencesGroup::builder()
            .title("Install and Removal")
            .description("Ask for confirmation before changing packages.")
            .build();
        let confirm_install_row = adw::ActionRow::builder()
            .title("Confirm before installing packages")
            .build();
        let confirm_install_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        confirm_install_switch.set_active(self.state.borrow().confirm_install);
        confirm_install_row.add_suffix(&confirm_install_switch);
        confirm_install_row.set_activatable_widget(Some(&confirm_install_switch));

        let confirm_remove_row = adw::ActionRow::builder()
            .title("Confirm before removing packages")
            .build();
        let confirm_remove_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        confirm_remove_switch.set_active(self.state.borrow().confirm_remove);
        confirm_remove_row.add_suffix(&confirm_remove_switch);
        confirm_remove_row.set_activatable_widget(Some(&confirm_remove_switch));

        install_group.add(&confirm_install_row);
        install_group.add(&confirm_remove_row);
        general_page.add(&install_group);

        prefs.add(&general_page);

        let controller_clone = Rc::clone(self);
        start_combo.connect_selected_notify(move |row| {
            let preference = if row.selected() == 1 {
                StartPagePreference::LastVisited
            } else {
                StartPagePreference::Discover
            };
            controller_clone.update_start_page_preference(preference);
        });

        let controller_clone = Rc::clone(self);
        let freq_combo_clone = freq_combo.clone();
        auto_switch.connect_active_notify(move |switcher| {
            let active = switcher.is_active();
            controller_clone.set_auto_check_enabled(active, true);
            freq_combo_clone.set_sensitive(active);
        });

        let controller_clone = Rc::clone(self);
        freq_combo.connect_selected_notify(move |row| {
            let frequency = if row.selected() == 1 {
                UpdateCheckFrequency::Weekly
            } else {
                UpdateCheckFrequency::Daily
            };
            controller_clone.set_auto_check_frequency(frequency, true);
        });

        let controller_clone = Rc::clone(self);
        confirm_install_switch.connect_active_notify(move |switcher| {
            controller_clone.set_confirm_install(switcher.is_active(), true);
        });

        let controller_clone = Rc::clone(self);
        confirm_remove_switch.connect_active_notify(move |switcher| {
            controller_clone.set_confirm_remove(switcher.is_active(), true);
        });

        let controller_clone = Rc::clone(self);
        notify_switch.connect_active_notify(move |switcher| {
            controller_clone.set_notify_updates(switcher.is_active(), true);
        });

        prefs.present();
    }

    fn begin_install(self: &Rc<Self>, package: PackageInfo) {
        self.execute_install(package);
    }

    fn execute_install(self: &Rc<Self>, package: PackageInfo) {
        {
            let mut state = self.state.borrow_mut();
            if state.install_in_progress {
                return;
            }
            state.install_in_progress = true;
        }

        self.rebuild_search_list();

        let message = format!("Installing \"{}\"…", package.name);
        self.set_footer_message(Some(&message));
        let sender = self.sender.clone();
        let package_name = package.name.clone();
        thread::spawn(move || {
            let result = run_xbps_install(&package_name);
            let _ = sender.send(AppMessage::InstallFinished {
                package: package_name,
                result,
            });
        });
    }

    fn execute_remove(self: &Rc<Self>, package: String, origin: RemoveOrigin) {
        {
            let mut state = self.state.borrow_mut();
            if state.remove_in_progress {
                return;
            }
            state.remove_in_progress = true;
        }

        let message = format!("Removing \"{}\"…", package);
        self.set_footer_message(Some(&message));

        match origin {
            RemoveOrigin::Discover => {}
            RemoveOrigin::Installed => {
                self.set_installed_status_message(Some(message.clone()));
                self.rebuild_installed_list();
            }
        }

        self.rebuild_search_list();

        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = run_xbps_remove(&package);
            let _ = sender.send(AppMessage::RemoveFinished { package, result });
        });
    }

    fn execute_remove_batch(self: &Rc<Self>, packages: Vec<String>) {
        if packages.is_empty() {
            return;
        }

        {
            let mut state = self.state.borrow_mut();
            if state.remove_in_progress {
                return;
            }
            state.remove_in_progress = true;
        }

        self.update_installed_selection_ui();

        let message = format!(
            "Removing {} selected package{}…",
            packages.len(),
            if packages.len() == 1 { "" } else { "s" }
        );
        self.set_installed_status_message(Some(message.clone()));
        self.set_footer_message(Some(&message));

        let sender = self.sender.clone();
        let packages_for_thread = packages.clone();
        thread::spawn(move || {
            let result = run_xbps_remove_packages(&packages_for_thread);
            let _ = sender.send(AppMessage::RemoveBatchFinished {
                packages: packages_for_thread,
                result,
            });
        });
    }

    fn on_view_changed(self: &Rc<Self>) {
        let current = self.widgets.view_stack.visible_child_name();
        match current.as_deref() {
            Some("discover") => if !self.widgets.discover_button.is_active() {},
            Some("installed") => {
                if !self.widgets.installed_button.is_active() {}
                if self.state.borrow().installed_packages.is_empty()
                    && !self.state.borrow().installed_refresh_in_progress
                {
                    self.refresh_installed_packages();
                }
            }
            Some("updates") => if !self.widgets.updates_button.is_active() {},
            _ => {}
        }

        if let Some(name) = current.as_deref() {
            self.record_last_page(name);
        }
    }

    fn on_discover_primary_action(self: &Rc<Self>) {
        let pkg = match self.current_search_selection() {
            Some(pkg) => pkg,
            None => return,
        };

        if pkg.installed {
            self.on_remove_from_discover_requested();
        } else {
            self.on_install_requested();
        }
    }

    fn on_search_requested(self: &Rc<Self>) {
        let query = self.widgets.discover.search_entry.text().trim().to_string();
        if query.is_empty() {
            self.set_discover_status(Some(
                "Type a package name or keyword to search the repository.",
            ));
            return;
        }

        let clear_category = {
            let state = self.state.borrow();
            state.active_spotlight_category.is_some()
        };
        if clear_category {
            self.clear_spotlight_category();
        }

        {
            let mut state = self.state.borrow_mut();
            if state.search_in_progress {
                return;
            }
            state.search_in_progress = true;
            state.discover_mode = DiscoverMode::Search;
        }

        self.update_discover_layout();

        let message = format!("Searching for \"{}\"…", query);
        self.set_discover_status(Some(&message));
        self.widgets.discover.search_button.set_sensitive(false);
        self.widgets.discover.search_spinner.set_visible(true);
        self.widgets.discover.search_spinner.start();
        let preserve_navigation = {
            let state = self.state.borrow();
            state.discover_detail_navigation_active || state.pending_discover_target.is_some()
        };
        self.clear_discover_details(preserve_navigation);
        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = run_xbps_query_search(&query);
            let _ = sender.send(AppMessage::SearchFinished { query, result });
        });
    }

    fn on_discover_search_changed(self: &Rc<Self>, text: String) {
        if !text.trim().is_empty() {
            return;
        }

        let should_reset = {
            let mut state = self.state.borrow_mut();
            if state.discover_mode != DiscoverMode::Search {
                false
            } else {
                state.discover_mode = DiscoverMode::Spotlight;
                state.search_results.clear();
                state.selected_search = None;
                state.search_in_progress = false;
                state.discover_detail_focus = None;
                true
            }
        };

        if !should_reset {
            return;
        }

        self.widgets.discover.search_spinner.stop();
        self.widgets.discover.search_spinner.set_visible(false);
        self.widgets.discover.search_button.set_sensitive(true);
        self.rebuild_search_list();
        self.clear_discover_details(false);
        self.update_discover_layout();
        self.set_discover_status(Some(
            "Type a package name or keyword to search the repository.",
        ));
    }

    fn on_install_requested(self: &Rc<Self>) {
        let package = match self.current_search_selection() {
            Some(pkg) if !pkg.installed => pkg,
            Some(pkg) => {
                let message = format!("\"{}\" is already installed.", pkg.name);
                self.set_discover_status(Some(&message));
                return;
            }
            None => return,
        };
        if self.state.borrow().confirm_install {
            let pkg_clone = package.clone();
            let heading = format!("Install \"{}\"?", package.name);
            let body = "Nebula will install this package and any required dependencies.";
            self.confirm_action(&heading, body, "Install", move |controller| {
                controller.begin_install(pkg_clone);
            });
            return;
        }

        self.begin_install(package);
    }

    fn on_remove_from_discover_requested(self: &Rc<Self>) {
        let package = match self.current_search_selection() {
            Some(pkg) if pkg.installed => pkg,
            _ => return,
        };
        self.start_remove(package.name, RemoveOrigin::Discover);
    }

    fn start_remove(self: &Rc<Self>, package: String, origin: RemoveOrigin) {
        if self.state.borrow().confirm_remove {
            let pkg_clone = package.clone();
            let heading = format!("Remove \"{}\"?", package);
            let body = "The package and its data will be removed from this system.";
            self.confirm_action(&heading, body, "Remove", move |controller| {
                controller.begin_remove(pkg_clone.clone(), origin);
            });
            return;
        }

        self.begin_remove(package, origin);
    }

    fn begin_remove(self: &Rc<Self>, package: String, origin: RemoveOrigin) {
        self.execute_remove(package, origin);
    }

    fn on_installed_search_changed(self: &Rc<Self>, query: String) {
        {
            let mut state = self.state.borrow_mut();
            state.installed_filter = query;
        }
        self.rebuild_installed_list();
    }

    fn on_installed_filter_changed(self: &Rc<Self>, selected: u32) {
        let filter = match selected {
            0 => InstalledFilter::All,
            1 => InstalledFilter::Updates,
            _ => InstalledFilter::All,
        };

        {
            let mut state = self.state.borrow_mut();
            if state.installed_filter_mode == filter {
                return;
            }
            state.installed_filter_mode = filter;
        }
        self.rebuild_installed_list();
    }

    fn on_installed_remove_selected(self: &Rc<Self>) {
        let packages = {
            let state = self.state.borrow();
            if state.remove_in_progress || state.installed_selected.is_empty() {
                return;
            }
            state.installed_selected.iter().cloned().collect::<Vec<_>>()
        };

        if packages.is_empty() {
            return;
        }

        self.execute_remove_batch(packages);
    }

    fn on_installed_row_selected(self: &Rc<Self>, row: Option<gtk::ListBoxRow>) {
        let navigation_triggered = {
            let mut state = self.state.borrow_mut();
            let nav = state.installed_detail_navigation_active;
            state.installed_detail_navigation_active = false;
            nav
        };

        {
            let mut state = self.state.borrow_mut();
            state.selected_installed = row.as_ref().map(|r| r.index() as usize);
        }

        if !navigation_triggered {
            self.clear_installed_detail_history();
        }

        if let Some(pkg) = row.and_then(|r| {
            let idx = r.index() as usize;
            let state = self.state.borrow();
            state
                .installed_filtered
                .get(idx)
                .and_then(|orig| state.installed_packages.get(*orig))
                .cloned()
        }) {
            self.request_installed_detail(&pkg.name);
        }
        self.update_installed_details();
    }

    fn on_installed_selection_toggled(self: &Rc<Self>, package: String, selected: bool) {
        {
            let mut state = self.state.borrow_mut();
            if selected {
                state.installed_selected.insert(package.clone());
            } else {
                state.installed_selected.remove(&package);
            }
        }
        self.update_installed_selection_ui();
    }

    fn on_installed_detail_remove(self: &Rc<Self>) {
        let package = {
            let state = self.state.borrow();
            state.installed_detail_package.clone()
        };

        if let Some(pkg) = package {
            self.start_remove(pkg, RemoveOrigin::Installed);
        }
    }

    fn on_installed_detail_update(self: &Rc<Self>) {
        let package = {
            let state = self.state.borrow();
            state.installed_detail_package.clone()
        };

        if let Some(pkg) = package {
            self.start_update(pkg, false);
        }
    }

    fn on_installed_detail_close(self: &Rc<Self>) {
        self.widgets.installed.list.unselect_all();
        self.clear_installed_detail_history();
        self.clear_installed_detail();
        self.update_installed_detail_back_button();
        self.update_installed_summary();
    }

    fn request_installed_detail(&self, package: &str) {
        let package_name = package.to_string();
        let installed_set = self.state.borrow().installed_set.clone();

        {
            let mut state = self.state.borrow_mut();
            if state.installed_detail_cache.contains_key(&package_name)
                || state.installed_detail_loading.contains(&package_name)
            {
                return;
            }
            state.installed_detail_errors.remove(&package_name);
            state.installed_detail_loading.insert(package_name.clone());
        }

        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = query_installed_detail(&package_name, &installed_set);
            let _ = sender.send(AppMessage::InstalledDetailsLoaded {
                package: package_name,
                result,
            });
        });
    }

    fn refresh_installed_packages(self: &Rc<Self>) {
        {
            let mut state = self.state.borrow_mut();
            if state.installed_refresh_in_progress {
                return;
            }
            state.installed_refresh_in_progress = true;
        }

        self.set_installed_status_message(Some("Refreshing installed packages…".to_string()));
        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = run_xbps_list_installed();
            let _ = sender.send(AppMessage::InstalledFinished { result });
        });
    }

    fn on_search_row_selected(self: &Rc<Self>, row: Option<gtk::ListBoxRow>) {
        let selected_index = row.as_ref().map(|r| r.index() as usize);
        let navigation = {
            let mut state = self.state.borrow_mut();
            let nav = state.discover_detail_navigation_active;
            if !nav {
                state.discover_detail_history.clear();
            }
            state.discover_detail_navigation_active = false;
            state.pending_discover_target = None;
            state.selected_search = selected_index;
            if let Some(idx) = selected_index {
                if let Some(pkg) = state.search_results.get(idx) {
                    state.discover_detail_focus = Some(pkg.clone());
                }
            } else if !nav {
                state.discover_detail_focus = None;
            }
            nav
        };
        if !navigation {
            self.update_discover_detail_back_button();
        }
        self.update_discover_details();
        self.update_spotlight_recent_detail();
    }

    fn handle_message(self: &Rc<Self>, msg: AppMessage) {
        match msg {
            AppMessage::SearchFinished { query, result } => {
                self.finish_search(query, result);
            }
            AppMessage::InstalledFinished { result } => {
                self.finish_installed_refresh(result);
            }
            AppMessage::InstallFinished { package, result } => {
                self.finish_install(package, result);
            }
            AppMessage::RemoveFinished { package, result } => {
                self.finish_remove(package, result);
            }
            AppMessage::RemoveBatchFinished { packages, result } => {
                self.finish_remove_batch(packages, result);
            }
            AppMessage::InstalledDetailsLoaded { package, result } => {
                self.finish_installed_detail(package, result);
            }
            AppMessage::UpdatesDetailLoaded { package, result } => {
                self.finish_updates_detail(package, result);
            }
            AppMessage::UpdatesRefreshed {
                packages,
                success,
                error,
            } => {
                self.finish_updates_refresh(packages, success, error);
            }
            AppMessage::UpdateFinished {
                packages,
                result,
                all,
            } => {
                self.finish_update(packages, result, all);
            }
            AppMessage::DiscoverDetailLoaded { package, result } => {
                self.finish_discover_detail(package, result);
            }
            AppMessage::SpotlightLoaded {
                recent,
                categories,
                cache,
                refreshed_at,
            } => {
                self.finish_spotlight_loaded(recent, categories, cache, refreshed_at);
            }
            AppMessage::SpotlightFailed { error } => {
                self.finish_spotlight_failed(error);
            }
        }
    }

    fn finish_search(self: &Rc<Self>, query: String, result: Result<Vec<PackageInfo>, String>) {
        self.widgets.discover.search_spinner.stop();
        self.widgets.discover.search_spinner.set_visible(false);
        self.widgets.discover.search_button.set_sensitive(true);
        {
            let mut state = self.state.borrow_mut();
            state.search_in_progress = false;
        }

        match result {
            Ok(mut packages) => {
                {
                    let state = self.state.borrow();
                    packages.iter_mut().for_each(|pkg| {
                        pkg.installed = state.installed_set.contains(&pkg.name);
                    });
                }

                let (pending_target, navigation_active) = {
                    let mut state = self.state.borrow_mut();
                    state.search_results = packages;
                    state.selected_search = None;
                    state.discover_detail_focus = None;
                    state.discover_detail_cache.clear();
                    state.discover_detail_loading.clear();
                    state.discover_detail_errors.clear();
                    state.discover_mode = DiscoverMode::Search;
                    (
                        state.pending_discover_target.clone(),
                        state.discover_detail_navigation_active,
                    )
                };

                let results_len = self.state.borrow().search_results.len();
                if results_len == 0 {
                    self.clear_discover_details(navigation_active);
                    let message = format!("No packages matched \"{}\".", query);
                    self.set_discover_status(Some(&message));
                } else {
                    let message = format!(
                        "Found {} package{} for \"{}\".",
                        results_len,
                        if results_len == 1 { "" } else { "s" },
                        query
                    );
                    self.set_discover_status(Some(&message));
                    self.rebuild_search_list();
                }
                self.update_discover_layout();

                if let Some(target) = pending_target {
                    if !self.focus_discover_package(&target, navigation_active) {
                        let mut state = self.state.borrow_mut();
                        state.pending_discover_target = None;
                        state.discover_detail_navigation_active = false;
                        self.update_discover_detail_back_button();
                    }
                }
            }
            Err(err) => {
                self.clear_search_results();
                let message = format!("Could not search: {}", err);
                self.set_discover_status(Some(&message));
                self.show_toast("Search failed.");
            }
        }
    }

    fn finish_installed_refresh(self: &Rc<Self>, result: Result<Vec<PackageInfo>, String>) {
        {
            let mut state = self.state.borrow_mut();
            state.installed_refresh_in_progress = false;
        }

        match result {
            Ok(packages) => {
                let mut state = self.state.borrow_mut();
                state.installed_set = packages.iter().map(|pkg| pkg.name.clone()).collect();
                state.installed_packages = packages;
                state.installed_last_refresh = glib::DateTime::now_local().ok();
                state.installed_selected.clear();
                state.selected_installed = None;
                drop(state);
                self.update_search_installed_flags();
                self.rebuild_installed_list();
                self.update_installed_selection_ui();
                self.update_spotlight_installed_flags();
                self.update_spotlight_views();
                self.set_installed_status_message(None);
            }
            Err(err) => {
                self.clear_installed_results();
                self.set_installed_status_message(Some(format!(
                    "Could not refresh installed packages: {}",
                    err
                )));
                self.show_toast("Failed to refresh installed packages.");
            }
        }
    }

    fn finish_install(self: &Rc<Self>, package: String, result: Result<CommandResult, String>) {
        {
            let mut state = self.state.borrow_mut();
            state.install_in_progress = false;
        }

        let footer_message = match result {
            Ok(command) => {
                if command.success() {
                    let message = format!("\"{}\" installed successfully.", package);
                    self.show_toast(&format!("Installed {}.", package));
                    self.flag_installed_state(&package, true);
                    self.refresh_installed_packages();
                    Some(message)
                } else {
                    let mut detail = command.stderr.trim();
                    if detail.is_empty() {
                        detail = command.stdout.trim();
                    }
                    let message = if detail.is_empty() {
                        format!("Failed to install \"{}\".", package)
                    } else {
                        format!("Failed to install \"{}\": {}", package, detail)
                    };
                    self.show_error_dialog("Install Failed", &message);
                    Some(message)
                }
            }
            Err(err) => {
                let message = format!("Failed to install \"{}\": {}", package, err);
                self.show_error_dialog("Install Failed", &message);
                Some(message)
            }
        };
        self.update_discover_details();
        self.refresh_updates(true);
        self.rebuild_search_list();
        if let Some(msg) = footer_message {
            self.set_footer_message(Some(&msg));
        }
    }

    fn finish_remove(self: &Rc<Self>, package: String, result: Result<CommandResult, String>) {
        {
            let mut state = self.state.borrow_mut();
            state.remove_in_progress = false;
            state.installed_selected.remove(&package);
            state.installed_detail_cache.remove(&package);
            state.installed_detail_loading.remove(&package);
            state.installed_detail_errors.remove(&package);
        }

        self.rebuild_installed_list();
        self.update_installed_selection_ui();

        let footer_message = match result {
            Ok(command) => {
                if command.success() {
                    let message = format!("\"{}\" removed successfully.", package);
                    self.set_installed_status_message(Some(message.clone()));
                    self.show_toast(&format!("Removed {}.", package));
                    self.flag_installed_state(&package, false);
                    self.refresh_installed_packages();
                    Some(message)
                } else {
                    let mut detail = command.stderr.trim();
                    if detail.is_empty() {
                        detail = command.stdout.trim();
                    }
                    let message = if detail.is_empty() {
                        format!("Failed to remove \"{}\".", package)
                    } else {
                        format!("Failed to remove \"{}\": {}", package, detail)
                    };
                    self.show_error_dialog("Removal Failed", &message);
                    Some(message)
                }
            }
            Err(err) => {
                let message = format!("Failed to remove \"{}\": {}", package, err);
                self.show_error_dialog("Removal Failed", &message);
                Some(message)
            }
        };
        self.update_discover_details();
        self.refresh_updates(true);
        self.rebuild_search_list();
        if let Some(msg) = footer_message {
            self.set_footer_message(Some(&msg));
        }
    }

    fn finish_remove_batch(
        self: &Rc<Self>,
        packages: Vec<String>,
        result: Result<CommandResult, String>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.remove_in_progress = false;
            for pkg in &packages {
                state.installed_selected.remove(pkg);
                state.installed_detail_cache.remove(pkg);
                state.installed_detail_loading.remove(pkg);
                state.installed_detail_errors.remove(pkg);
            }
        }

        self.rebuild_installed_list();
        self.update_installed_selection_ui();

        let footer_message = match result {
            Ok(command) => {
                if command.success() {
                    let message = if packages.len() == 1 {
                        format!("\"{}\" removed successfully.", packages[0])
                    } else {
                        "Selected packages removed successfully.".to_string()
                    };
                    self.set_installed_status_message(Some(message.clone()));
                    if packages.len() == 1 {
                        self.show_toast(&format!("Removed {}.", packages[0]));
                    } else {
                        self.show_toast("Selected packages removed.");
                    }
                    for pkg in packages {
                        self.flag_installed_state(&pkg, false);
                    }
                    self.refresh_installed_packages();
                    Some(message)
                } else {
                    let mut detail = command.stderr.trim();
                    if detail.is_empty() {
                        detail = command.stdout.trim();
                    }
                    let message = if detail.is_empty() {
                        "Failed to remove selected packages.".to_string()
                    } else {
                        format!("Failed to remove selected packages: {}", detail)
                    };
                    self.show_error_dialog("Removal Failed", &message);
                    Some(message)
                }
            }
            Err(err) => {
                let message = format!("Failed to remove selected packages: {}", err);
                self.show_error_dialog("Removal Failed", &message);
                Some(message)
            }
        };
        self.refresh_updates(true);
        if let Some(msg) = footer_message {
            self.set_footer_message(Some(&msg));
        }
    }

    fn finish_installed_detail(
        self: &Rc<Self>,
        package: String,
        result: Result<InstalledDetail, String>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.installed_detail_loading.remove(&package);
            match result {
                Ok(detail) => {
                    state.installed_detail_errors.remove(&package);
                    state.installed_detail_cache.insert(package.clone(), detail);
                }
                Err(err) => {
                    state.installed_detail_errors.insert(package.clone(), err);
                }
            }
        }

        self.update_installed_details();
    }

    fn finish_updates_detail(
        self: &Rc<Self>,
        package: String,
        result: Result<InstalledDetail, String>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.updates_detail_loading.remove(&package);
            match result {
                Ok(detail) => {
                    state.updates_detail_errors.remove(&package);
                    state.updates_detail_cache.insert(package.clone(), detail);
                }
                Err(err) => {
                    state.updates_detail_errors.insert(package.clone(), err);
                }
            }
        }

        self.update_updates_detail();
    }

    fn finish_discover_detail(
        self: &Rc<Self>,
        package: String,
        result: Result<DiscoverDetail, String>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.discover_detail_loading.remove(&package);
            match result {
                Ok(detail) => {
                    let cloned = detail.clone();
                    state.discover_detail_errors.remove(&package);
                    state.discover_detail_cache.insert(package.clone(), detail);

                    if let Some(pkg) = state
                        .search_results
                        .iter_mut()
                        .find(|pkg| pkg.name == package)
                    {
                        if let Some(description) = cloned.description.clone() {
                            if !description.is_empty() {
                                pkg.set_description(description);
                            }
                        }
                        if let Some(download) = cloned.download.clone() {
                            pkg.download_size = Some(download);
                        } else if let Some(bytes) = cloned.download_bytes {
                            pkg.download_size = Some(format_size(bytes));
                        }
                        if let Some(bytes) = cloned.download_bytes {
                            pkg.download_bytes = Some(bytes);
                        }
                        if let Some(repo) = cloned.repository.clone() {
                            pkg.repository = Some(repo);
                        }
                    }

                    let installed_flag = state.installed_set.contains(&package);
                    if let Some(focus) = state.discover_detail_focus.as_mut() {
                        if focus.name == package {
                            if let Some(ver) = cloned.version.clone() {
                                focus.set_version(ver);
                            }
                            if let Some(description) = cloned.description.clone() {
                                if !description.is_empty() {
                                    focus.set_description(description);
                                }
                            }
                            focus.download_bytes = cloned.download_bytes;
                            focus.download_size = cloned
                                .download
                                .clone()
                                .or_else(|| cloned.download_bytes.map(format_size));
                            focus.repository = cloned.repository.clone();
                            focus.installed = installed_flag;
                        }
                    }
                }
                Err(err) => {
                    state.discover_detail_errors.insert(package.clone(), err);
                }
            }
        }

        self.update_discover_details();
        self.update_spotlight_recent_detail();
    }

    fn show_toast(&self, message: &str) {
        let toast = adw::Toast::builder().title(message).timeout(5).build();
        self.widgets.toast_overlay.add_toast(toast);
    }

    fn maybe_notify_new_updates(&self, count: usize) {
        if count == 0 {
            return;
        }

        let should_notify = {
            let state = self.state.borrow();
            state.notify_updates && !state.updates_notification_sent
        };

        if !should_notify {
            return;
        }

        let summary = "New updates available!";
        let body = if count == 1 {
            "1 update is ready to install.".to_string()
        } else {
            format!("{} updates are ready to install.", count)
        };

        let notification = gio::Notification::new(summary);
        notification.set_body(Some(&body));
        notification.set_default_action("app.show-updates");
        let icon = gio::ThemedIcon::new("software-update-available");
        notification.set_icon(&icon);

        self.app.send_notification(Some("updates"), &notification);

        if let Ok(mut state) = self.state.try_borrow_mut() {
            state.updates_notification_sent = true;
        }
    }

    fn withdraw_updates_notification(&self) {
        self.app.withdraw_notification("updates");
    }

    fn show_error_dialog(&self, title: &str, message: &str) {
        let dialog = gtk::MessageDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .message_type(gtk::MessageType::Error)
            .text(title)
            .secondary_text(message)
            .build();
        dialog.add_button("Close", gtk::ResponseType::Close);
        dialog.connect_response(|dlg, _| dlg.close());
        dialog.present();
    }

    fn set_discover_status(&self, _message: Option<&str>) {
        // No-op; messages now routed to footer
    }

    fn set_footer_message(&self, message: Option<&str>) {
        {
            let mut state = self.state.borrow_mut();
            state.footer_message = message.map(|m| m.to_string());
        }
        self.update_footer_text();
    }

    fn set_check_buttons_sensitive(&self, enabled: bool) {
        self.widgets.updates.check_button.set_sensitive(enabled);
        self.widgets.updates.refresh_button.set_sensitive(enabled);
        self.widgets
            .updates
            .update_all_button
            .set_sensitive(enabled);
    }

    fn set_status_text(&self, text: &str) {
        let label = &self.widgets.updates.status_label;
        label.set_text(text);
        label.set_visible(!text.is_empty());
    }

    fn set_summary_text(&self, text: &str) {
        self.widgets.updates.summary_label.set_text(text);
        let should_show = !text.is_empty() || self.widgets.updates.spinner.is_visible();
        self.widgets
            .updates
            .summary_label
            .set_visible(!text.is_empty());
        self.widgets.updates.summary_row.set_visible(should_show);
    }

    fn set_installed_status_message(&self, message: Option<String>) {
        {
            let mut state = self.state.borrow_mut();
            state.installed_status_message = message;
        }
        self.update_installed_summary();
    }

    fn update_installed_summary(&self) {
        let (
            total,
            filtered,
            refreshing,
            last_refresh,
            message,
            remove_in_progress,
            selected_count,
        ) = {
            let state = self.state.borrow();
            (
                state.installed_packages.len(),
                state.installed_filtered.len(),
                state.installed_refresh_in_progress,
                state.installed_last_refresh.clone(),
                state.installed_status_message.clone(),
                state.remove_in_progress,
                state.installed_selected.len(),
            )
        };

        let status_text = if refreshing {
            message.unwrap_or_else(|| "Refreshing installed packages…".to_string())
        } else if let Some(text) = message {
            text
        } else if total == 0 {
            "No packages are installed yet. Install something from Discover.".to_string()
        } else if filtered == total {
            format!(
                "{} installed package{} found.",
                total,
                if total == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Showing {} of {} installed package{}.",
                filtered,
                total,
                if total == 1 { "" } else { "s" }
            )
        };

        let footer_text = if let Some(dt) = last_refresh {
            if let Some(chrono_dt) = glib_datetime_to_chrono(&dt) {
                format!("Last refreshed {}", format_relative_time(chrono_dt))
            } else {
                "Last refreshed just now".to_string()
            }
        } else {
            "Last refreshed —".to_string()
        };

        if refreshing {
            self.widgets.installed.spinner.set_visible(true);
            self.widgets.installed.spinner.start();
        } else {
            self.widgets.installed.spinner.stop();
            self.widgets.installed.spinner.set_visible(false);
        }

        self.widgets.installed.status_label.set_text(&status_text);
        self.widgets.installed.status_label.set_visible(true);
        self.widgets.installed.footer_label.set_text(&footer_text);

        let can_remove = selected_count > 0 && !remove_in_progress && !refreshing;
        self.widgets
            .installed
            .remove_selected_button
            .set_sensitive(can_remove);

        let (detail_pkg, updates_busy) = {
            let state = self.state.borrow();
            (
                state.installed_detail_package.clone(),
                state.update_in_progress || state.updates_loading,
            )
        };
        if let Some(pkg) = detail_pkg {
            self.widgets
                .installed
                .detail_remove_button
                .set_visible(true);
            self.widgets
                .installed
                .detail_remove_button
                .set_sensitive(!remove_in_progress && !refreshing);

            let has_update = {
                let state = self.state.borrow();
                state.available_updates.iter().any(|p| p.name == pkg)
            };
            if has_update {
                self.widgets
                    .installed
                    .detail_update_button
                    .set_visible(true);
                self.widgets
                    .installed
                    .detail_update_button
                    .set_sensitive(!updates_busy && !refreshing);
            } else {
                self.widgets
                    .installed
                    .detail_update_button
                    .set_visible(false);
            }
        } else {
            self.widgets
                .installed
                .detail_remove_button
                .set_visible(false);
            self.widgets
                .installed
                .detail_update_button
                .set_visible(false);
        }
    }

    fn update_installed_selection_ui(&self) {
        self.update_installed_summary();
    }

    fn update_installed_details(self: &Rc<Self>) {
        let (maybe_pkg, updates) = {
            let state = self.state.borrow();
            let updates: HashSet<String> = state
                .available_updates
                .iter()
                .map(|pkg| pkg.name.clone())
                .collect();
            let selected = state.selected_installed.and_then(|list_idx| {
                state
                    .installed_filtered
                    .get(list_idx)
                    .and_then(|orig_idx| state.installed_packages.get(*orig_idx))
                    .cloned()
            });
            (selected, updates)
        };

        if let Some(pkg) = maybe_pkg.clone() {
            {
                let mut state = self.state.borrow_mut();
                state.installed_detail_package = Some(pkg.name.clone());
            }
            self.widgets.installed.detail_frame.set_visible(true);
            self.widgets.installed.detail_close_button.set_visible(true);
            self.widgets
                .installed
                .detail_close_button
                .set_sensitive(true);
            self.widgets
                .installed
                .detail_stack
                .set_visible_child_name("detail");
            self.widgets
                .installed
                .detail_name
                .set_text(pkg.name.as_str());
            self.widgets
                .installed
                .detail_version_value
                .set_text(pkg.version.as_str());
            let has_update = updates.contains(&pkg.name);
            self.widgets
                .installed
                .detail_update_label
                .set_visible(false);
            self.widgets.installed.detail_update_label.set_text("");

            let (remove_in_progress, updates_busy) = {
                let state = self.state.borrow();
                (
                    state.remove_in_progress,
                    state.update_in_progress || state.updates_loading,
                )
            };

            self.widgets
                .installed
                .detail_remove_button
                .set_visible(true);
            self.widgets
                .installed
                .detail_remove_button
                .set_sensitive(!remove_in_progress);

            if has_update {
                self.widgets
                    .installed
                    .detail_update_button
                    .set_visible(true);
                self.widgets
                    .installed
                    .detail_update_button
                    .set_sensitive(!updates_busy);
            } else {
                self.widgets
                    .installed
                    .detail_update_button
                    .set_visible(false);
            }

            let (detail, loading, error) = {
                let state = self.state.borrow();
                (
                    state.installed_detail_cache.get(&pkg.name).cloned(),
                    state.installed_detail_loading.contains(&pkg.name),
                    state.installed_detail_errors.get(&pkg.name).cloned(),
                )
            };

            if detail.is_none() && !loading && error.is_none() {
                self.request_installed_detail(&pkg.name);
            }

            let description_body = if let Some(detail_ref) = detail.as_ref() {
                detail_ref
                    .long_description
                    .as_ref()
                    .cloned()
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| {
                        if pkg.description.is_empty() {
                            "This package does not provide a description.".to_string()
                        } else {
                            pkg.description.clone()
                        }
                    })
            } else if pkg.description.is_empty() {
                "This package does not provide a description.".to_string()
            } else {
                pkg.description.clone()
            };
            self.widgets
                .installed
                .detail_description
                .set_text(&description_body);

            let download_value = if let Some(detail) = detail.as_ref() {
                if let Some(err) = detail.download_error.as_ref() {
                    format!("Failed ({})", err)
                } else if let Some(formatted) = detail.download_formatted.as_ref() {
                    formatted.clone()
                } else if let Some(bytes) = detail.download_bytes {
                    format_download_size(bytes)
                } else {
                    "Unknown".to_string()
                }
            } else if loading {
                "Loading…".to_string()
            } else if let Some(err) = error.as_ref() {
                format!("Failed ({})", err)
            } else {
                "—".to_string()
            };
            self.widgets
                .installed
                .detail_download_value
                .set_text(&download_value);

            if let Some(detail_ref) = detail.as_ref() {
                if let Some(home) = detail_ref
                    .homepage
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    let widgets = &self.widgets.installed;
                    widgets.detail_homepage_row.set_visible(true);
                    widgets.detail_homepage_link.set_visible(true);
                    widgets.detail_homepage_link.set_label(home);
                    widgets.detail_homepage_link.set_uri(home);
                    widgets.detail_homepage_link.set_tooltip_text(Some(home));
                } else {
                    let widgets = &self.widgets.installed;
                    widgets.detail_homepage_link.set_visible(false);
                    widgets.detail_homepage_row.set_visible(false);
                }

                if let Some(maint) = detail_ref
                    .maintainer
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    let widgets = &self.widgets.installed;
                    let friendly = sanitize_contact_field(maint);
                    if friendly.is_empty() {
                        widgets.detail_maintainer_value.set_visible(false);
                        widgets.detail_maintainer_row.set_visible(false);
                    } else {
                        widgets.detail_maintainer_row.set_visible(true);
                        widgets.detail_maintainer_value.set_visible(true);
                        widgets.detail_maintainer_value.set_text(&friendly);
                    }
                } else {
                    let widgets = &self.widgets.installed;
                    widgets.detail_maintainer_value.set_visible(false);
                    widgets.detail_maintainer_row.set_visible(false);
                }

                if let Some(license) = detail_ref
                    .license
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    let widgets = &self.widgets.installed;
                    widgets.detail_license_row.set_visible(true);
                    widgets.detail_license_value.set_visible(true);
                    widgets.detail_license_value.set_text(license);
                } else {
                    let widgets = &self.widgets.installed;
                    widgets.detail_license_row.set_visible(false);
                    widgets.detail_license_value.set_visible(false);
                    widgets.detail_license_value.set_text("");
                }
            } else {
                let widgets = &self.widgets.installed;
                widgets.detail_homepage_link.set_visible(false);
                widgets.detail_homepage_row.set_visible(false);
                widgets.detail_maintainer_value.set_visible(false);
                widgets.detail_maintainer_row.set_visible(false);
                widgets.detail_license_value.set_visible(false);
                widgets.detail_license_row.set_visible(false);
            }

            self.update_installed_required_by_ui(detail.as_ref(), loading, error.as_ref());
            self.set_installed_row_buttons_visible(false);
        } else {
            self.clear_installed_detail();
        }
        self.update_installed_detail_back_button();
        self.update_installed_summary();
    }

    fn clear_installed_detail(self: &Rc<Self>) {
        {
            let mut state = self.state.borrow_mut();
            state.installed_detail_package = None;
        }
        let widgets = &self.widgets.installed;
        widgets.detail_stack.set_visible_child_name("placeholder");
        widgets.detail_frame.set_visible(false);
        widgets.detail_close_button.set_visible(false);
        widgets.detail_close_button.set_sensitive(false);
        widgets.detail_name.set_text("Select a package");
        widgets
            .detail_description
            .set_text("Select a package to see details.");
        widgets.detail_download_value.set_text("—");
        widgets.detail_version_value.set_text("—");
        widgets.detail_update_label.set_visible(false);
        widgets.detail_update_label.set_text("");
        widgets.detail_homepage_link.set_visible(false);
        widgets.detail_homepage_link.set_label("");
        widgets.detail_homepage_link.set_uri("");
        widgets.detail_homepage_link.set_tooltip_text(None);
        widgets.detail_homepage_row.set_visible(false);
        widgets.detail_maintainer_value.set_visible(false);
        widgets.detail_maintainer_value.set_text("");
        widgets.detail_maintainer_row.set_visible(false);
        widgets.detail_license_value.set_visible(false);
        widgets.detail_license_value.set_text("");
        widgets.detail_license_row.set_visible(false);
        widgets.detail_remove_button.set_visible(false);
        widgets.detail_remove_button.set_sensitive(false);
        widgets.detail_update_button.set_visible(false);
        widgets.detail_update_button.set_sensitive(false);
        self.set_installed_row_buttons_visible(true);
        self.update_installed_required_by_ui(None, false, None);
    }

    fn set_installed_row_buttons_visible(&self, visible: bool) {
        for button in self.installed_buttons.borrow().iter() {
            button.set_visible(visible);
        }
    }

    fn update_installed_required_by_ui(
        self: &Rc<Self>,
        detail: Option<&InstalledDetail>,
        loading: bool,
        detail_error: Option<&String>,
    ) {
        let installed_widgets = &self.widgets.installed;
        clear_listbox(&installed_widgets.detail_required_by_list);

        if let Some(detail) = detail {
            if let Some(err) = detail.required_by_error.as_ref() {
                installed_widgets
                    .detail_required_by_placeholder
                    .set_text(&format!("Failed to load ({})", err));
                installed_widgets.detail_required_by_list.set_visible(false);
                installed_widgets
                    .detail_required_by_stack
                    .set_visible_child_name("placeholder");
                return;
            }

            if detail.required_by.is_empty() {
                installed_widgets
                    .detail_required_by_placeholder
                    .set_text("Not required by any installed package.");
                installed_widgets.detail_required_by_list.set_visible(false);
                installed_widgets
                    .detail_required_by_stack
                    .set_visible_child_name("placeholder");
                return;
            }

            for dependent in &detail.required_by {
                let row = adw::ActionRow::builder().title(dependent.as_str()).build();
                row.set_activatable(true);
                row.set_focusable(true);

                let status_label = gtk::Label::builder()
                    .label("Installed")
                    .halign(gtk::Align::End)
                    .valign(gtk::Align::Center)
                    .build();
                status_label.add_css_class("dim-label");
                row.add_suffix(&status_label);

                let package_name = dependent.clone();
                row.connect_activated(glib::clone!(@strong self as controller => move |_| {
                    controller.on_installed_required_by_clicked(package_name.clone());
                }));

                installed_widgets.detail_required_by_list.append(&row);
            }

            installed_widgets.detail_required_by_list.set_visible(true);
            installed_widgets
                .detail_required_by_stack
                .set_visible_child_name("list");
            return;
        }

        let placeholder = if loading {
            "Loading…".to_string()
        } else if let Some(err) = detail_error {
            format!("Failed to load ({})", err)
        } else {
            "Not required by any installed package.".to_string()
        };

        installed_widgets
            .detail_required_by_placeholder
            .set_text(&placeholder);
        installed_widgets.detail_required_by_list.set_visible(false);
        installed_widgets
            .detail_required_by_stack
            .set_visible_child_name("placeholder");
    }

    fn clear_installed_detail_history(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.installed_detail_history.clear();
        }
        self.update_installed_detail_back_button();
    }

    fn update_installed_detail_back_button(&self) {
        let has_history = !self.state.borrow().installed_detail_history.is_empty();
        let button = &self.widgets.installed.detail_back_button;
        button.set_visible(has_history);
        button.set_sensitive(has_history);
    }

    fn on_installed_detail_back(self: &Rc<Self>) {
        let previous = {
            let mut state = self.state.borrow_mut();
            state.installed_detail_history.pop()
        };

        if let Some(package) = previous {
            if !self.focus_installed_package(&package, true) {
                let mut state = self.state.borrow_mut();
                state.installed_detail_navigation_active = false;
                state.installed_detail_history.push(package);
                self.show_toast("Package is hidden by current filters.");
            }
        }

        self.update_installed_detail_back_button();
    }

    fn on_installed_required_by_clicked(self: &Rc<Self>, package: String) {
        let mut pushed = false;
        {
            let mut state = self.state.borrow_mut();
            if let Some(current) = state.installed_detail_package.clone() {
                if current != package {
                    state.installed_detail_history.push(current);
                    pushed = true;
                }
            }
            state.installed_detail_navigation_active = true;
        }

        if !self.focus_installed_package(&package, true) {
            let mut state = self.state.borrow_mut();
            state.installed_detail_navigation_active = false;
            if pushed {
                state.installed_detail_history.pop();
            }
            self.show_toast("Package is hidden by current filters.");
        }
        self.update_installed_detail_back_button();
    }

    fn focus_installed_package(self: &Rc<Self>, package: &str, navigation: bool) -> bool {
        if navigation {
            self.state.borrow_mut().installed_detail_navigation_active = true;
        }

        let target_index = {
            let state = self.state.borrow();
            state
                .installed_packages
                .iter()
                .position(|pkg| pkg.name == package)
        };

        let Some(target_index) = target_index else {
            if navigation {
                self.state.borrow_mut().installed_detail_navigation_active = false;
            }
            return false;
        };

        let mut filtered_index = {
            let state = self.state.borrow();
            state
                .installed_filtered
                .iter()
                .position(|orig| *orig == target_index)
        };

        if filtered_index.is_none() {
            if self.widgets.installed.filter_dropdown.selected() != 0 {}
            if !self.widgets.installed.search_entry.text().is_empty() {}
            self.rebuild_installed_list();
            filtered_index = {
                let state = self.state.borrow();
                state
                    .installed_filtered
                    .iter()
                    .position(|orig| *orig == target_index)
            };
        }

        let Some(list_index) = filtered_index else {
            if navigation {
                self.state.borrow_mut().installed_detail_navigation_active = false;
            }
            return false;
        };

        {
            let mut state = self.state.borrow_mut();
            state.selected_installed = Some(list_index);
        }

        if let Some(row) = self.widgets.installed.list.row_at_index(list_index as i32) {
            if row.parent().is_some() {
                row.grab_focus();
            }
        }

        set_toggle_button_state(&self.widgets.installed_button, true);

        true
    }

    fn update_discover_detail_back_button(&self) {
        let has_history = !self.state.borrow().discover_detail_history.is_empty();
        let button = &self.widgets.discover.detail_back_button;
        button.set_visible(has_history);
        button.set_sensitive(has_history);
    }

    fn focus_discover_package(self: &Rc<Self>, package: &str, navigation: bool) -> bool {
        if navigation {
            self.state.borrow_mut().discover_detail_navigation_active = true;
        }

        let position = {
            let state = self.state.borrow();
            state
                .search_results
                .iter()
                .position(|pkg| pkg.name == package)
        };

        let Some(idx) = position else {
            return false;
        };

        {
            let mut state = self.state.borrow_mut();
            state.selected_search = Some(idx);
            state.pending_discover_target = None;
            if let Some(pkg) = state.search_results.get(idx) {
                state.discover_detail_focus = Some(pkg.clone());
            }
        }

        if let Some(row) = self.widgets.discover.list.row_at_index(idx as i32) {
            if row.parent().is_some() {
                row.grab_focus();
            }
        } else {
            self.update_discover_details();
        }

        true
    }

    fn on_discover_dependency_clicked(self: &Rc<Self>, package: String) {
        let package = package.trim();
        if package.is_empty() {
            return;
        }
        let package = package.to_string();

        {
            let mut state = self.state.borrow_mut();
            if let Some(current) = state.discover_detail_package.clone() {
                if current == package {
                    state.discover_detail_navigation_active = false;
                    state.pending_discover_target = None;
                    drop(state);
                    self.update_discover_detail_back_button();
                    return;
                }
                state.discover_detail_history.push(current);
            }
            state.discover_detail_navigation_active = true;
            state.pending_discover_target = Some(package.clone());
        }

        if self.focus_discover_package(&package, true) {
            self.update_discover_detail_back_button();
            return;
        }

        self.open_discover_dependency_detail(package);
    }

    fn open_discover_dependency_detail(self: &Rc<Self>, package: String) {
        let installed = {
            let state = self.state.borrow();
            state.installed_set.contains(&package)
        };

        {
            let mut state = self.state.borrow_mut();
            state.discover_detail_navigation_active = true;
            state.selected_search = None;
            state.pending_discover_target = None;
            let version = String::new();
            let description = String::new();
            state.discover_detail_focus = Some(PackageInfo {
                name_lower: lowercase_cache(&package),
                version_lower: lowercase_cache(&version),
                description_lower: lowercase_cache(&description),
                name: package.clone(),
                version,
                description,
                installed,
                previous_version: None,
                download_size: None,
                changelog: None,
                download_bytes: None,
                repository: None,
                build_date: None,
                first_seen: None,
            });
            state.discover_detail_package = Some(package.clone());
        }

        self.update_discover_details();
        self.request_discover_detail(&package);
        self.update_discover_detail_back_button();
    }

    fn set_discover_row_buttons_visible(&self, visible: bool) {
        for button in self.discover_buttons.borrow().iter() {
            button.set_visible(visible);
        }
    }

    fn on_discover_detail_back(self: &Rc<Self>) {
        let previous = {
            let mut state = self.state.borrow_mut();
            state.discover_detail_history.pop()
        };

        if let Some(package) = previous {
            {
                let mut state = self.state.borrow_mut();
                state.discover_detail_navigation_active = true;
                state.pending_discover_target = Some(package.clone());
            }

            if self.focus_discover_package(&package, true) {
                self.update_discover_detail_back_button();
                return;
            }

            self.open_discover_dependency_detail(package);
            return;
        } else {
            self.state.borrow_mut().discover_detail_navigation_active = false;
        }

        self.update_discover_detail_back_button();
    }

    fn cancel_auto_check_timer(&self) {
        if let Some(source) = self.state.borrow_mut().auto_check_source.take() {
            source.remove();
        }
    }

    fn clear_auto_check_handle(&self) {
        self.state.borrow_mut().auto_check_source = None;
    }

    fn schedule_auto_check(self: &Rc<Self>) {
        self.cancel_auto_check_timer();

        let (enabled, frequency) = {
            let state = self.state.borrow();
            (state.auto_check_enabled, state.auto_check_frequency)
        };

        if !enabled {
            return;
        }

        let interval = match frequency {
            UpdateCheckFrequency::Daily => 24 * 60 * 60,
            UpdateCheckFrequency::Weekly => 7 * 24 * 60 * 60,
        };

        let weak_self = Rc::downgrade(self);
        let source_id = glib::timeout_add_seconds_local(interval, move || {
            if let Some(controller) = weak_self.upgrade() {
                if !controller.state.borrow().auto_check_enabled {
                    controller.clear_auto_check_handle();
                    return glib::ControlFlow::Break;
                }

                controller.trigger_auto_check_from_timer();
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });

        self.state.borrow_mut().auto_check_source = Some(source_id);
    }

    fn trigger_auto_check_from_timer(self: &Rc<Self>) {
        if !self.can_trigger_auto_check_now() {
            return;
        }
        self.refresh_updates(true);
    }

    fn can_trigger_auto_check_now(&self) -> bool {
        let state = self.state.borrow();
        !state.updates_loading && !state.update_in_progress
    }

    fn rebuild_search_list(self: &Rc<Self>) {
        let list = &self.widgets.discover.list;
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        let (results, selected_idx, pending_target, navigation_active) = {
            let state = self.state.borrow();
            (
                state.search_results.clone(),
                state.selected_search,
                state.pending_discover_target.clone(),
                state.discover_detail_navigation_active,
            )
        };
        self.discover_buttons.borrow_mut().clear();
        for pkg in &results {
            let row = self.build_discover_row(pkg);
            list.append(&row);
        }

        if let Some(idx) = selected_idx {
            if let Some(row) = list.row_at_index(idx as i32) {
                list.select_row(Some(&row));
            }
        } else if let Some(target) = pending_target {
            let _ = self.focus_discover_package(&target, navigation_active);
        } else {
            list.unselect_all();
        }
        self.update_discover_details();
    }

    fn build_discover_row(self: &Rc<Self>, pkg: &PackageInfo) -> adw::ActionRow {
        let title = glib::markup_escape_text(&pkg.name);
        let version_line = if pkg.version.is_empty() {
            "Version —".to_string()
        } else {
            format!("Version {}", glib::markup_escape_text(&pkg.version))
        };
        let description = if pkg.description.is_empty() {
            "No description provided".to_string()
        } else {
            glib::markup_escape_text(&pkg.description).to_string()
        };

        let subtitle = format!("{}\n{}", version_line, description);

        let row = adw::ActionRow::builder()
            .title(title.as_str())
            .subtitle(subtitle.as_str())
            .build();
        row.set_activatable(false);
        row.set_focusable(false);
        row.set_title_lines(1);
        row.set_subtitle_lines(2);

        let button = gtk::Button::builder().width_request(140).build();
        button.set_valign(gtk::Align::Center);

        {
            let state = self.state.borrow();
            if pkg.installed {
                button.set_label("Installed");
                button.add_css_class("pill");
                button.set_tooltip_text(Some("Already installed."));
            } else {
                button.set_label("Install");
                button.add_css_class("suggested-action");
                button.set_sensitive(!state.install_in_progress);
                button.set_tooltip_text(Some("Install this package."));
            }
        }

        let package_name = pkg.name.clone();
        let weak_self = Rc::downgrade(self);
        if !pkg.installed {
            button.connect_clicked(move |_| {
                if let Some(controller) = weak_self.upgrade() {
                    controller.select_search_row_by_name(&package_name);
                    controller.on_discover_primary_action();
                }
            });
        }

        row.add_suffix(&button);
        self.discover_buttons.borrow_mut().push(button);

        row
    }

    fn rebuild_installed_list(self: &Rc<Self>) {
        let list = &self.widgets.installed.list;
        clear_listbox(list);
        self.installed_buttons.borrow_mut().clear();

        let status_message;
        let selected_index;

        {
            let mut state = self.state.borrow_mut();
            let filter_lower = state.installed_filter.to_lowercase();
            let filter_mode = state.installed_filter_mode;
            let remove_in_progress = state.remove_in_progress;
            let total_installed = state.installed_packages.len();

            let mut matched: Vec<usize> = state
                .installed_packages
                .iter()
                .enumerate()
                .filter(|(_, pkg)| package_matches_filter(pkg, &filter_lower))
                .filter(|(_, pkg)| {
                    filter_mode != InstalledFilter::Updates
                        || state.available_update_names.contains(&pkg.name)
                })
                .map(|(idx, _)| idx)
                .collect();

            matched.sort_by(|a, b| {
                let pkg_a = &state.installed_packages[*a];
                let pkg_b = &state.installed_packages[*b];
                pkg_a.name.cmp(&pkg_b.name)
            });

            state.installed_filtered = matched.clone();
            if let Some(selected) = state.selected_installed {
                if selected >= matched.len() {
                    state.selected_installed = None;
                }
            }
            selected_index = state.selected_installed;

            for idx in &matched {
                let pkg = &state.installed_packages[*idx];
                let is_selected = state.installed_selected.contains(&pkg.name);
                let has_update = state.available_update_names.contains(&pkg.name);
                let row =
                    self.build_installed_row(pkg, remove_in_progress, is_selected, has_update);
                list.append(&row);
            }

            let filtered_count = matched.len();
            status_message = if total_installed == 0 {
                Some("No packages are installed yet. Install something from Discover.".to_string())
            } else if filtered_count == 0 {
                if filter_mode == InstalledFilter::Updates {
                    Some("No installed packages have updates available.".to_string())
                } else {
                    Some("No installed packages match your search.".to_string())
                }
            } else if filter_lower.trim().is_empty() && filter_mode == InstalledFilter::All {
                None
            } else {
                Some(format!(
                    "Showing {} package{} matching your filters.",
                    filtered_count,
                    if filtered_count == 1 { "" } else { "s" }
                ))
            };
        }

        if let Some(selected_idx) = selected_index {
            if let Some(row) = list.row_at_index(selected_idx as i32) {
                list.select_row(Some(&row));
            }
        }

        self.set_installed_status_message(status_message);

        self.update_installed_selection_ui();
        self.update_installed_details();
    }

    fn build_installed_row(
        self: &Rc<Self>,
        pkg: &PackageInfo,
        remove_disabled: bool,
        selected: bool,
        has_update: bool,
    ) -> adw::ActionRow {
        let title = glib::markup_escape_text(&pkg.name);
        let version = glib::markup_escape_text(&pkg.version);
        let description = if pkg.description.is_empty() {
            glib::markup_escape_text("No description provided").to_string()
        } else {
            glib::markup_escape_text(&pkg.description).to_string()
        };

        let row = adw::ActionRow::builder()
            .title(title.as_str())
            .subtitle(format!("Version {}\n{}", version, description).as_str())
            .build();
        row.set_activatable(false);
        row.set_focusable(false);
        row.set_title_lines(1);
        row.set_subtitle_lines(2);

        if let Some(changelog) = &pkg.changelog {
            if !changelog.is_empty() {
                row.set_tooltip_text(Some(changelog));
            }
        }

        let check_button = gtk::CheckButton::builder().active(selected).build();
        check_button.set_valign(gtk::Align::Center);
        check_button.set_sensitive(!remove_disabled);
        let package_name = pkg.name.clone();
        check_button.connect_toggled(glib::clone!(@strong self as controller => move |btn| {
            controller.on_installed_selection_toggled(package_name.clone(), btn.is_active());
        }));
        row.add_prefix(&check_button);

        if has_update {
            let update_button = gtk::Button::builder().label("Update").build();
            update_button.add_css_class("suggested-action");
            update_button.set_valign(gtk::Align::Center);
            let package_name = pkg.name.clone();
            let weak_self = Rc::downgrade(self);
            update_button.connect_clicked(move |_| {
                if let Some(controller) = weak_self.upgrade() {
                    controller.start_update(package_name.clone(), false);
                }
            });
            self.installed_buttons
                .borrow_mut()
                .push(update_button.clone());
            row.add_suffix(&update_button);
        }

        let remove_button = gtk::Button::builder().label("Remove").build();
        remove_button.add_css_class("destructive-action");
        remove_button.set_sensitive(!remove_disabled);
        remove_button.set_valign(gtk::Align::Center);

        let package_name = pkg.name.clone();
        let weak_self = Rc::downgrade(self);
        remove_button.connect_clicked(move |_| {
            if let Some(controller) = weak_self.upgrade() {
                controller.start_remove(package_name.clone(), RemoveOrigin::Installed);
            }
        });

        self.installed_buttons
            .borrow_mut()
            .push(remove_button.clone());
        row.add_suffix(&remove_button);

        row
    }

    fn rebuild_updates_list(self: &Rc<Self>) {
        let list = &self.widgets.updates.list;
        clear_listbox(list);

        let (updates, selected, busy, detail_open) = {
            let state = self.state.borrow();
            (
                state.available_updates.clone(),
                state.selected_updates.clone(),
                state.update_in_progress || state.updates_loading,
                state.updates_detail_package.is_some(),
            )
        };
        self.update_buttons.borrow_mut().clear();

        for pkg in &updates {
            let is_selected = selected.contains(&pkg.name);
            let row = self.build_update_row(pkg, busy, detail_open, is_selected);
            list.append(&row);
        }

        let detail_target = {
            let state = self.state.borrow();
            state.updates_detail_package.clone()
        };

        if let Some(target) = detail_target {
            if let Some(idx) = updates.iter().position(|pkg| pkg.name == target) {
                if let Some(row) = list.row_at_index(idx as i32) {
                    list.select_row(Some(&row));
                }
                let mut state = self.state.borrow_mut();
                state.selected_update = Some(idx);
            } else {
                self.clear_updates_detail();
                list.unselect_all();
            }
        } else {
            list.unselect_all();
        }
    }

    fn build_update_row(
        self: &Rc<Self>,
        pkg: &PackageInfo,
        disabled: bool,
        detail_open: bool,
        selected: bool,
    ) -> adw::ActionRow {
        let title = glib::markup_escape_text(&pkg.name);
        let subtitle = if pkg.description.is_empty() {
            glib::markup_escape_text("Update available").to_string()
        } else {
            glib::markup_escape_text(&pkg.description).to_string()
        };

        let row = adw::ActionRow::builder()
            .title(title.as_str())
            .subtitle(subtitle.as_str())
            .build();
        row.set_activatable(false);
        row.set_focusable(false);
        row.set_title_lines(1);
        row.set_subtitle_lines(2);

        let version_label_text = if let Some(prev) = &pkg.previous_version {
            if prev.is_empty() {
                pkg.version.clone()
            } else if pkg.version.is_empty() {
                prev.clone()
            } else {
                format!("{} → {}", prev, pkg.version)
            }
        } else {
            pkg.version.clone()
        };

        let check_button = gtk::CheckButton::builder().active(selected).build();
        check_button.set_sensitive(!disabled);
        check_button.set_valign(gtk::Align::Center);
        let package_name = pkg.name.clone();
        check_button.connect_toggled(glib::clone!(@strong self as controller => move |btn| {
            controller.on_update_selection_changed(package_name.clone(), btn.is_active());
        }));
        row.add_prefix(&check_button);

        if !version_label_text.is_empty() {
            let version_label = gtk::Label::new(Some(version_label_text.as_str()));
            version_label.add_css_class("dim-label");
            version_label.set_halign(gtk::Align::End);
            version_label.set_valign(gtk::Align::Center);
            version_label.set_margin_end(12);
            row.add_suffix(&version_label);
        }

        let update_button = gtk::Button::builder().label("Update").build();
        update_button.add_css_class("suggested-action");
        update_button.set_sensitive(!disabled);
        update_button.set_valign(gtk::Align::Center);
        update_button.set_margin_start(12);
        update_button.set_visible(!detail_open);

        let package_name = pkg.name.clone();
        update_button.connect_clicked(glib::clone!(@strong self as controller => move |_| {
            controller.start_update(package_name.clone(), false);
        }));

        row.add_suffix(&update_button);
        self.update_buttons.borrow_mut().push(update_button.clone());

        row
    }

    fn on_update_selection_changed(self: &Rc<Self>, package: String, selected: bool) {
        {
            let mut state = self.state.borrow_mut();
            if selected {
                state.selected_updates.insert(package.clone());
            } else {
                state.selected_updates.remove(&package);
            }
        }
        self.update_update_controls();
    }

    fn update_update_controls(self: &Rc<Self>) {
        let (total, selected, loading, updating) = {
            let state = self.state.borrow();
            (
                state.available_updates.len(),
                state.selected_updates.len(),
                state.updates_loading,
                state.update_in_progress,
            )
        };

        self.update_summary_text();

        if total == 0 {
            return;
        }

        let label = if selected == 0 {
            "Update Selected".to_string()
        } else if selected == total {
            format!("Update All ({})", total)
        } else {
            format!("Update Selected ({})", selected)
        };
        self.widgets
            .updates
            .update_all_button
            .set_sensitive(selected > 0 && !loading && !updating);
        self.widgets.updates.update_all_button.set_label(&label);

        self.update_updates_detail();
    }

    fn update_summary_text(&self) {
        let summary = {
            let state = self.state.borrow();
            let count = state.available_updates.len();
            if count > 0 {
                let total_bytes = state.total_update_size;
                if total_bytes > 0 {
                    let megabytes = total_bytes as f64 / 1_000_000.0;
                    format!("Update size {:.2} MB", megabytes)
                } else {
                    String::new()
                }
            } else if state.last_update_check.is_some() {
                String::new()
            } else {
                "No updates checked yet.".to_string()
            }
        };

        self.set_summary_text(&summary);
    }

    fn update_footer_text(&self) {
        let text = {
            let state = self.state.borrow();
            if let Some(message) = state.footer_message.clone() {
                message
            } else if state.updates_loading {
                "Checking for updates…".to_string()
            } else if let Some(dt) = &state.last_update_check {
                if let Some(chrono_dt) = glib_datetime_to_chrono(dt) {
                    format!("Last checked {}", format_relative_time(chrono_dt))
                } else {
                    "Last checked just now".to_string()
                }
            } else {
                "Last checked — never.".to_string()
            }
        };

        self.widgets.updates.footer_label.set_text(&text);
    }

    fn on_update_row_activated(self: &Rc<Self>, row: &gtk::ListBoxRow) {
        let index = row.index() as usize;
        let package = {
            let state = self.state.borrow();
            if state.update_in_progress || state.updates_loading {
                return;
            }
            state
                .available_updates
                .get(index)
                .map(|pkg| pkg.name.clone())
        };

        if let Some(name) = package {
            self.start_update(name, false);
        }
    }

    fn on_update_row_selected(self: &Rc<Self>, row: Option<gtk::ListBoxRow>) {
        let (index, package) = {
            let state = self.state.borrow();
            let idx = row.as_ref().map(|r| r.index() as usize);
            let pkg = idx.and_then(|i| state.available_updates.get(i).cloned());
            (idx, pkg)
        };

        {
            let mut state = self.state.borrow_mut();
            state.selected_update = index;
            state.updates_detail_package = package.as_ref().map(|pkg| pkg.name.clone());
        }

        if let Some(pkg) = package {
            self.request_updates_detail(&pkg.name);
            self.update_updates_detail();
        } else {
            self.clear_updates_detail();
        }
    }

    fn on_updates_detail_update(self: &Rc<Self>) {
        let package = {
            let state = self.state.borrow();
            state.updates_detail_package.clone()
        };

        if let Some(pkg) = package {
            self.start_update(pkg, false);
        }
    }

    fn request_updates_detail(&self, package: &str) {
        let installed_set = {
            let state = self.state.borrow();
            state.installed_set.clone()
        };

        let package_name = package.to_string();
        {
            let mut state = self.state.borrow_mut();
            if state.updates_detail_cache.contains_key(&package_name)
                || state.updates_detail_loading.contains(&package_name)
            {
                return;
            }
            state.updates_detail_errors.remove(&package_name);
            state.updates_detail_loading.insert(package_name.clone());
        }

        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = query_installed_detail(&package_name, &installed_set);
            let _ = sender.send(AppMessage::UpdatesDetailLoaded {
                package: package_name,
                result,
            });
        });
    }

    fn clear_updates_detail(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.updates_detail_package = None;
            state.selected_update = None;
        }

        let widgets = &self.widgets.updates;
        widgets
            .detail_description
            .set_text("Select an update to see details.");
        clear_listbox(&widgets.detail_required_by_list);
        widgets
            .detail_required_by_placeholder
            .set_text("Not required by any installed package.");
        widgets
            .detail_required_by_stack
            .set_visible_child_name("placeholder");
        widgets.detail_frame.set_visible(false);
        widgets.detail_close_button.set_visible(false);
        widgets.detail_close_button.set_sensitive(false);
        widgets.detail_update_button.set_visible(false);
        widgets.detail_update_button.set_sensitive(false);
        self.set_all_update_row_buttons_visible(true);
    }

    fn update_updates_detail(self: &Rc<Self>) {
        let (package_name, pkg_info, detail, loading, error) = {
            let state = self.state.borrow();
            let name = state.updates_detail_package.clone();
            let pkg = name
                .as_ref()
                .and_then(|pkg_name| {
                    state
                        .available_updates
                        .iter()
                        .find(|pkg| &pkg.name == pkg_name)
                })
                .cloned();
            let detail = name
                .as_ref()
                .and_then(|pkg_name| state.updates_detail_cache.get(pkg_name).cloned());
            let loading = name
                .as_ref()
                .map(|pkg_name| state.updates_detail_loading.contains(pkg_name))
                .unwrap_or(false);
            let error = name
                .as_ref()
                .and_then(|pkg_name| state.updates_detail_errors.get(pkg_name).cloned());
            (name, pkg, detail, loading, error)
        };

        if let Some(pkg_name) = package_name {
            if pkg_info.is_none() && !loading {
                self.clear_updates_detail();
                return;
            }

            let widgets = &self.widgets.updates;
            widgets.placeholder.set_visible(false);
            widgets.content_row.set_visible(true);
            widgets.detail_frame.set_visible(true);
            widgets.detail_stack.set_visible_child_name("detail");
            widgets.detail_close_button.set_visible(true);
            widgets.detail_close_button.set_sensitive(true);
            self.set_all_update_row_buttons_visible(false);

            let display_name = pkg_info
                .as_ref()
                .map(|pkg| pkg.name.as_str())
                .unwrap_or(pkg_name.as_str());
            widgets.detail_name.set_text(display_name);

            let version_text = pkg_info
                .as_ref()
                .map(|pkg| {
                    if pkg.version.is_empty() {
                        "—".to_string()
                    } else {
                        pkg.version.clone()
                    }
                })
                .unwrap_or_else(|| "—".to_string());
            widgets.detail_version_value.set_text(&version_text);

            let download_text = if let Some(detail_ref) = detail.as_ref() {
                if let Some(err) = detail_ref.download_error.as_ref() {
                    format!("Failed ({})", err)
                } else if let Some(formatted) = detail_ref.download_formatted.as_ref() {
                    formatted.clone()
                } else if let Some(bytes) = detail_ref.download_bytes {
                    format_download_size(bytes)
                } else if let Some(pkg) = pkg_info.as_ref() {
                    pkg.download_size.clone().unwrap_or_else(|| "—".to_string())
                } else {
                    "—".to_string()
                }
            } else if loading {
                "Loading…".to_string()
            } else if let Some(err) = error.as_ref() {
                format!("Failed ({})", err)
            } else if let Some(pkg) = pkg_info.as_ref() {
                pkg.download_size.clone().unwrap_or_else(|| "—".to_string())
            } else {
                "—".to_string()
            };
            widgets.detail_download_value.set_text(&download_text);

            let description_body = if let Some(detail_ref) = detail.as_ref() {
                detail_ref
                    .long_description
                    .as_ref()
                    .cloned()
                    .filter(|text| !text.trim().is_empty())
                    .unwrap_or_else(|| {
                        pkg_info
                            .as_ref()
                            .map(|pkg| pkg.description.clone())
                            .filter(|desc| !desc.trim().is_empty())
                            .unwrap_or_else(|| {
                                "This package does not provide a description.".to_string()
                            })
                    })
            } else if let Some(pkg) = pkg_info.as_ref() {
                if pkg.description.is_empty() {
                    "This package does not provide a description.".to_string()
                } else {
                    pkg.description.clone()
                }
            } else {
                "This package does not provide a description.".to_string()
            };
            widgets.detail_description.set_text(&description_body);

            if let Some(detail_ref) = detail.as_ref() {
                if let Some(home) = detail_ref
                    .homepage
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    widgets.detail_homepage_row.set_visible(true);
                    widgets.detail_homepage_link.set_visible(true);
                    widgets.detail_homepage_link.set_label(home);
                    widgets.detail_homepage_link.set_uri(home);
                    widgets.detail_homepage_link.set_tooltip_text(Some(home));
                } else {
                    widgets.detail_homepage_row.set_visible(false);
                    widgets.detail_homepage_link.set_visible(false);
                }

                if let Some(maint) = detail_ref
                    .maintainer
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    let friendly = sanitize_contact_field(maint);
                    if friendly.is_empty() {
                        widgets.detail_maintainer_row.set_visible(false);
                        widgets.detail_maintainer_value.set_visible(false);
                    } else {
                        widgets.detail_maintainer_row.set_visible(true);
                        widgets.detail_maintainer_value.set_visible(true);
                        widgets.detail_maintainer_value.set_text(&friendly);
                    }
                } else {
                    widgets.detail_maintainer_row.set_visible(false);
                    widgets.detail_maintainer_value.set_visible(false);
                }

                if let Some(license) = detail_ref
                    .license
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    widgets.detail_license_row.set_visible(true);
                    widgets.detail_license_value.set_visible(true);
                    widgets.detail_license_value.set_text(license);
                } else {
                    widgets.detail_license_row.set_visible(false);
                    widgets.detail_license_value.set_visible(false);
                }
            } else {
                widgets.detail_homepage_row.set_visible(false);
                widgets.detail_homepage_link.set_visible(false);
                widgets.detail_maintainer_row.set_visible(false);
                widgets.detail_maintainer_value.set_visible(false);
                widgets.detail_license_row.set_visible(false);
                widgets.detail_license_value.set_visible(false);
            }

            widgets.detail_update_label.set_visible(false);
            widgets.detail_update_button.set_sensitive(!loading);
            widgets.detail_update_button.set_visible(pkg_info.is_some());

            self.update_updates_required_by_ui(detail.as_ref(), loading, error.as_ref());
        } else {
            self.clear_updates_detail();
        }
    }

    fn set_all_update_row_buttons_visible(&self, visible: bool) {
        for button in self.update_buttons.borrow().iter() {
            button.set_visible(visible);
        }
    }

    fn on_updates_detail_close(self: &Rc<Self>) {
        self.widgets.updates.list.unselect_all();
        self.clear_updates_detail();
    }

    fn update_updates_required_by_ui(
        self: &Rc<Self>,
        detail: Option<&InstalledDetail>,
        loading: bool,
        detail_error: Option<&String>,
    ) {
        let widgets = &self.widgets.updates;
        clear_listbox(&widgets.detail_required_by_list);

        if let Some(detail) = detail {
            if let Some(err) = detail.required_by_error.as_ref() {
                widgets
                    .detail_required_by_placeholder
                    .set_text(&format!("Failed to load ({})", err));
                widgets
                    .detail_required_by_stack
                    .set_visible_child_name("placeholder");
                return;
            }

            if detail.required_by.is_empty() {
                widgets
                    .detail_required_by_placeholder
                    .set_text("Not required by any installed package.");
                widgets
                    .detail_required_by_stack
                    .set_visible_child_name("placeholder");
                return;
            }

            for dependent in &detail.required_by {
                let row = adw::ActionRow::builder().title(dependent.as_str()).build();
                row.set_activatable(false);
                row.set_focusable(false);

                let status_label = gtk::Label::builder()
                    .label("Installed")
                    .halign(gtk::Align::End)
                    .valign(gtk::Align::Center)
                    .build();
                status_label.add_css_class("dim-label");
                row.add_suffix(&status_label);

                widgets.detail_required_by_list.append(&row);
            }

            widgets.detail_required_by_list.set_visible(true);
            widgets
                .detail_required_by_stack
                .set_visible_child_name("list");
            return;
        }

        let placeholder = if loading {
            "Loading…".to_string()
        } else if let Some(err) = detail_error {
            format!("Failed to load ({})", err)
        } else {
            "Not required by any installed package.".to_string()
        };

        widgets
            .detail_required_by_placeholder
            .set_text(&placeholder);
        widgets
            .detail_required_by_stack
            .set_visible_child_name("placeholder");
    }

    fn sync_updates_detail_state(&self) {
        let mut state = self.state.borrow_mut();
        let available_names = state.available_update_names.clone();

        if let Some(target) = state.updates_detail_package.clone() {
            if let Some(idx) = state
                .available_updates
                .iter()
                .position(|pkg| pkg.name == target)
            {
                state.selected_update = Some(idx);
            } else {
                state.selected_update = None;
                state.updates_detail_package = None;
            }
        } else {
            state.selected_update = None;
        }

        state
            .updates_detail_cache
            .retain(|name, _| available_names.contains(name));
        state
            .updates_detail_errors
            .retain(|name, _| available_names.contains(name));
        state
            .updates_detail_loading
            .retain(|name| available_names.contains(name));
    }

    fn refresh_available_update_names(state: &mut AppState) {
        state.available_update_names.clear();
        state
            .available_update_names
            .extend(state.available_updates.iter().map(|pkg| pkg.name.clone()));
    }

    fn refresh_updates(self: &Rc<Self>, silent: bool) {
        self.set_footer_message(None);
        {
            let state = self.state.borrow();
            if state.update_in_progress || state.updates_loading {
                return;
            }
        }

        {
            let mut state = self.state.borrow_mut();
            state.updates_loading = true;
        }

        self.update_footer_text();

        let visible_updates =
            self.widgets.view_stack.visible_child_name() == Some("updates".into());

        if !silent || visible_updates {
            let message = if silent {
                "Refreshing updates…"
            } else {
                "Checking for updates…"
            };
            self.widgets.updates.placeholder_label.set_text(message);
            self.set_summary_text(message);
            self.set_status_text("");
        }

        if !silent {
            self.widgets.updates.placeholder.set_visible(true);
            self.widgets.updates.scroller.set_visible(false);
            self.widgets.updates.update_all_button.set_visible(false);
            self.widgets.updates.content_row.set_visible(false);
        }
        self.widgets.updates.spinner.set_visible(true);
        self.widgets.updates.spinner.start();

        self.set_check_buttons_sensitive(false);
        self.rebuild_updates_list();

        let sender = self.sender.clone();
        thread::spawn(move || match run_xbps_check_updates() {
            Ok(packages) => {
                let _ = sender.send(AppMessage::UpdatesRefreshed {
                    packages,
                    success: true,
                    error: None,
                });
            }
            Err(err) => {
                let _ = sender.send(AppMessage::UpdatesRefreshed {
                    packages: Vec::new(),
                    success: false,
                    error: Some(err),
                });
            }
        });
    }

    fn finish_updates_refresh(
        self: &Rc<Self>,
        packages: Vec<PackageInfo>,
        success: bool,
        error: Option<String>,
    ) {
        let (
            available,
            update_in_progress,
            should_notify,
            notify_count,
            footer_update,
            withdraw_notification,
        ) = {
            let mut state = self.state.borrow_mut();
            let had_updates = !state.available_updates.is_empty();
            state.updates_loading = false;
            if success {
                state.available_updates = packages;
                Self::refresh_available_update_names(&mut state);
                state.selected_updates = state
                    .available_updates
                    .iter()
                    .map(|pkg| pkg.name.clone())
                    .collect();
                state.total_update_size = state
                    .available_updates
                    .iter()
                    .filter_map(|pkg| pkg.download_bytes)
                    .sum();
                state.last_update_check = glib::DateTime::now_local().ok();
            }
            let available_names = state.available_update_names.clone();
            state
                .selected_updates
                .retain(|name| available_names.contains(name));
            let new_count = state.available_updates.len();
            let withdraw_notification = success && new_count == 0;
            if withdraw_notification {
                state.updates_notification_sent = false;
            }
            let should_notify = success
                && !had_updates
                && new_count > 0
                && state.notify_updates
                && !state.updates_notification_sent;
            let footer_update = if success && new_count > 0 {
                Some(if new_count == 1 {
                    "You have 1 update ready to install.".to_string()
                } else {
                    format!("You have {} updates ready to install.", new_count)
                })
            } else {
                None
            };
            let available = state.available_updates.clone();
            let update_in_progress = state.update_in_progress;
            (
                available,
                update_in_progress,
                should_notify,
                new_count,
                footer_update,
                withdraw_notification,
            )
        };

        self.widgets.updates.spinner.stop();
        self.widgets.updates.spinner.set_visible(false);

        if withdraw_notification {
            self.withdraw_updates_notification();
        }

        self.set_check_buttons_sensitive(!update_in_progress);

        if success {
            self.sync_updates_detail_state();
        }

        if !success {
            let message = error.unwrap_or_else(|| "Failed to check for updates.".to_string());
            self.set_status_text(&message);
            self.set_summary_text(&message);
            self.set_footer_message(Some(&message));
            self.widgets
                .updates
                .placeholder_label
                .set_text("Press “Check for updates” to try again.");
            self.widgets.updates.placeholder.set_visible(true);
            self.widgets.updates.scroller.set_visible(false);
            self.widgets.updates.update_all_button.set_visible(false);
            self.widgets.updates.content_row.set_visible(false);
            self.clear_updates_detail();
            self.widgets
                .updates
                .footer_label
                .set_text("Last check failed.");
            self.rebuild_updates_list();
            self.update_update_controls();
            self.update_updates_badge();
            return;
        }

        if available.is_empty() {
            self.set_status_text("");
            self.widgets
                .updates
                .placeholder_label
                .set_text("Your system is up to date!");
            self.clear_updates_detail();
        } else {
            self.set_status_text("");
        }

        let has_updates = !available.is_empty();
        self.widgets.updates.placeholder.set_visible(!has_updates);
        self.widgets.updates.scroller.set_visible(has_updates);
        self.widgets
            .updates
            .update_all_button
            .set_visible(has_updates);
        self.widgets.updates.content_row.set_visible(has_updates);

        if let Some(text) = footer_update {
            self.set_footer_message(Some(&text));
        } else if available.is_empty() {
            self.set_footer_message(None);
        }

        if should_notify {
            self.maybe_notify_new_updates(notify_count);
        }

        self.rebuild_updates_list();
        self.update_update_controls();
        self.update_updates_badge();
        self.update_footer_text();
        self.rebuild_installed_list();
        self.update_installed_summary();
    }

    fn update_all_packages(self: &Rc<Self>) {
        let (total, selected, loading, updating, packages) = {
            let state = self.state.borrow();
            (
                state.available_updates.len(),
                state.selected_updates.len(),
                state.updates_loading,
                state.update_in_progress,
                state.selected_updates.iter().cloned().collect::<Vec<_>>(),
            )
        };

        if total == 0 {
            self.widgets
                .updates
                .placeholder_label
                .set_text("Your system is up to date!");
            self.clear_updates_detail();
            return;
        }

        if loading || updating {
            return;
        }

        if selected == 0 {
            self.set_status_text("Select at least one update to apply.");
            return;
        }

        if selected == total {
            self.start_update(String::from("__all__"), true);
        } else {
            self.start_update_multiple(packages);
        }
    }

    fn start_update(self: &Rc<Self>, package: String, from_all: bool) {
        self.execute_update(package, from_all);
    }

    fn start_update_multiple(self: &Rc<Self>, packages: Vec<String>) {
        if packages.is_empty() {
            return;
        }
        self.execute_update_multiple(packages);
    }

    fn execute_update(self: &Rc<Self>, package: String, from_all: bool) {
        {
            let state = self.state.borrow();
            if state.update_in_progress || state.updates_loading {
                return;
            }
        }

        {
            let mut state = self.state.borrow_mut();
            state.update_in_progress = true;
        }

        let footer_message = if from_all {
            let message = "Installing all available updates…".to_string();
            self.set_status_text(&message);
            self.set_summary_text(&message);
            message
        } else {
            let message = format!("Updating \"{}\"…", package);
            self.set_status_text(&message);
            self.set_summary_text(&message);
            message
        };
        self.set_footer_message(Some(&footer_message));

        self.set_check_buttons_sensitive(false);

        self.rebuild_updates_list();
        self.update_updates_detail();

        let affected_packages = if from_all {
            self.state
                .borrow()
                .available_updates
                .iter()
                .map(|pkg| pkg.name.clone())
                .collect::<Vec<_>>()
        } else {
            vec![package.clone()]
        };

        let sender = self.sender.clone();
        if from_all {
            let packages_for_thread = affected_packages.clone();
            thread::spawn(move || {
                let result = run_xbps_update_all();
                let _ = sender.send(AppMessage::UpdateFinished {
                    packages: packages_for_thread,
                    result,
                    all: true,
                });
            });
        } else {
            let package_for_thread = package.clone();
            thread::spawn(move || {
                let result = run_xbps_update_package(&package_for_thread);
                let _ = sender.send(AppMessage::UpdateFinished {
                    packages: vec![package_for_thread],
                    result,
                    all: false,
                });
            });
        }
    }

    fn execute_update_multiple(self: &Rc<Self>, packages: Vec<String>) {
        if packages.is_empty() {
            return;
        }

        {
            let state = self.state.borrow();
            if state.update_in_progress || state.updates_loading {
                return;
            }
        }

        {
            let mut state = self.state.borrow_mut();
            state.update_in_progress = true;
        }

        let message = format!(
            "Updating {} selected package{}…",
            packages.len(),
            if packages.len() == 1 { "" } else { "s" }
        );
        self.set_status_text(&message);
        self.set_summary_text(&message);
        self.set_footer_message(Some(&message));
        self.set_check_buttons_sensitive(false);

        self.rebuild_updates_list();
        self.update_updates_detail();

        let affected = packages.clone();
        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = run_xbps_update_packages(&packages);
            let _ = sender.send(AppMessage::UpdateFinished {
                packages: affected,
                result,
                all: false,
            });
        });
    }

    fn finish_update(
        self: &Rc<Self>,
        packages: Vec<String>,
        result: Result<CommandResult, String>,
        all: bool,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.update_in_progress = false;
        }

        self.set_check_buttons_sensitive(true);

        match result {
            Ok(command) => {
                if command.success() {
                    if all {
                        let message = "System updated successfully.";
                        self.set_status_text(message);
                        self.set_summary_text(message);
                        self.set_footer_message(Some(message));
                        self.show_toast("All updates installed.");
                        {
                            let mut state = self.state.borrow_mut();
                            state.available_updates.clear();
                            Self::refresh_available_update_names(&mut state);
                            state.selected_updates.clear();
                            state.total_update_size = 0;
                        }
                    } else if packages.len() == 1 {
                        let name = packages.first().cloned().unwrap_or_default();
                        let message = format!("\"{}\" updated successfully.", name);
                        self.set_status_text(&message);
                        self.set_summary_text(&message);
                        self.set_footer_message(Some(&message));
                        self.show_toast(&format!("Updated {}.", name));
                        {
                            let mut state = self.state.borrow_mut();
                            state.available_updates.retain(|pkg| pkg.name != name);
                            Self::refresh_available_update_names(&mut state);
                            state.selected_updates.remove(&name);
                            state.total_update_size = state
                                .available_updates
                                .iter()
                                .filter_map(|pkg| pkg.download_bytes)
                                .sum();
                        }
                    } else {
                        let message = "Selected updates installed successfully.";
                        self.set_status_text(message);
                        self.set_summary_text(message);
                        self.set_footer_message(Some(message));
                        self.show_toast("Selected updates installed.");
                        {
                            let mut state = self.state.borrow_mut();
                            state
                                .available_updates
                                .retain(|pkg| !packages.contains(&pkg.name));
                            Self::refresh_available_update_names(&mut state);
                            state
                                .selected_updates
                                .retain(|name| !packages.contains(name));
                            state.total_update_size = state
                                .available_updates
                                .iter()
                                .filter_map(|pkg| pkg.download_bytes)
                                .sum();
                        }
                    }
                    self.refresh_installed_packages();
                    self.sync_updates_detail_state();
                    self.rebuild_updates_list();
                    self.update_update_controls();
                    self.update_updates_badge();
                    let mut withdraw_notification = false;
                    {
                        let mut state = self.state.borrow_mut();
                        if state.available_updates.is_empty() {
                            state.updates_notification_sent = false;
                            withdraw_notification = true;
                        }
                    }
                    if withdraw_notification {
                        self.withdraw_updates_notification();
                    }
                    self.refresh_updates(true);
                } else {
                    let mut detail = command.stderr.trim();
                    if detail.is_empty() {
                        detail = command.stdout.trim();
                    }
                    let message = if detail.is_empty() {
                        if all {
                            "Failed to install updates.".to_string()
                        } else if packages.len() == 1 {
                            format!("Failed to update \"{}\".", packages[0])
                        } else {
                            "Failed to install selected updates.".to_string()
                        }
                    } else if all {
                        format!("Failed to install updates: {}", detail)
                    } else if packages.len() == 1 {
                        format!("Failed to update \"{}\": {}", packages[0], detail)
                    } else {
                        format!("Failed to install selected updates: {}", detail)
                    };
                    self.set_status_text("");
                    self.set_summary_text("");
                    self.set_footer_message(Some(&message));
                    self.show_error_dialog("Update Failed", &message);
                    self.rebuild_updates_list();
                    self.update_update_controls();
                }
            }
            Err(err) => {
                let message = if all {
                    format!("Failed to install updates: {}", err)
                } else if packages.len() == 1 {
                    format!("Failed to update \"{}\": {}", packages[0], err)
                } else {
                    format!("Failed to install selected updates: {}", err)
                };
                self.set_status_text("");
                self.set_summary_text("");
                self.set_footer_message(Some(&message));
                self.show_error_dialog("Update Failed", &message);
                self.rebuild_updates_list();
                self.update_update_controls();
            }
        }

        let has_updates = {
            let state = self.state.borrow();
            !state.available_updates.is_empty()
        };

        self.widgets
            .updates
            .update_all_button
            .set_visible(has_updates);
        self.widgets
            .updates
            .update_all_button
            .set_sensitive(has_updates);
        if has_updates {
        } else {
            self.widgets
                .updates
                .placeholder_label
                .set_text("Your system is up to date!");
            self.clear_updates_detail();
        }

        self.update_updates_badge();
        self.update_footer_text();
    }

    fn finish_spotlight_loaded(
        self: &Rc<Self>,
        recent: Vec<PackageInfo>,
        categories: HashMap<SpotlightCategory, Vec<PackageInfo>>,
        cache: SpotlightCache,
        refreshed_at: DateTime<Utc>,
    ) {
        let cache_snapshot = {
            let mut state = self.state.borrow_mut();
            state.spotlight_loading = false;
            state.spotlight_recent = recent;
            state.spotlight_categories = categories;
            state.spotlight_cache = cache;
            state.spotlight_last_refresh = Some(refreshed_at);
            if let Some(selected) = state.spotlight_recent_selected.clone() {
                if !state
                    .spotlight_recent
                    .iter()
                    .any(|pkg| pkg.name == selected)
                {
                    state.spotlight_recent_selected = None;
                }
            }
            state.spotlight_cache.clone()
        };

        if let Err(err) = save_spotlight_cache_to_disk(&cache_snapshot) {
            eprintln!("Failed to persist spotlight cache: {}", err);
        }

        self.update_spotlight_installed_flags();
        self.update_spotlight_views();
        self.refresh_active_spotlight_category();
        self.update_discover_layout();
    }

    fn finish_spotlight_failed(self: &Rc<Self>, error: String) {
        {
            let mut state = self.state.borrow_mut();
            state.spotlight_loading = false;
        }

        eprintln!("Spotlight refresh failed: {}", error);
        self.update_spotlight_views();
        self.widgets
            .discover
            .spotlight_status
            .set_text(&format!("Could not refresh spotlight: {}", error));
    }

    fn initialize_spotlight(self: &Rc<Self>) {
        self.update_spotlight_installed_flags();
        self.update_spotlight_views();
        self.update_discover_layout();
        self.maybe_refresh_spotlight(false);
    }

    fn maybe_refresh_spotlight(self: &Rc<Self>, force: bool) {
        let should_refresh = {
            let state = self.state.borrow();
            if state.spotlight_loading {
                return;
            }
            let has_cached_data = !state.spotlight_recent.is_empty();
            if force {
                true
            } else if !has_cached_data {
                true
            } else if let Some(last) = state.spotlight_last_refresh {
                let delta = Utc::now().signed_duration_since(last);
                delta.num_hours() >= SPOTLIGHT_REFRESH_INTERVAL_HOURS
            } else {
                true
            }
        };

        if !should_refresh {
            return;
        }

        {
            let mut state = self.state.borrow_mut();
            state.spotlight_loading = true;
        }
        self.update_spotlight_views();

        let cache = {
            let state = self.state.borrow();
            state.spotlight_cache.clone()
        };
        let sender = self.sender.clone();
        thread::spawn(move || match refresh_spotlight_cache(cache) {
            Ok(outcome) => {
                let _ = sender.send(AppMessage::SpotlightLoaded {
                    recent: outcome.recent,
                    categories: outcome.categories,
                    cache: outcome.cache,
                    refreshed_at: outcome.refreshed_at,
                });
            }
            Err(error) => {
                let _ = sender.send(AppMessage::SpotlightFailed { error });
            }
        });
    }

    fn set_category_button_state(self: &Rc<Self>, active: Option<SpotlightCategory>) {
        let widgets = &self.widgets.discover;
        set_toggle_button_state(
            &widgets.category_browsers_button,
            active == Some(SpotlightCategory::Browsers),
        );
        set_toggle_button_state(
            &widgets.category_chat_button,
            active == Some(SpotlightCategory::Chat),
        );
        set_toggle_button_state(
            &widgets.category_email_button,
            active == Some(SpotlightCategory::Email),
        );
        set_toggle_button_state(
            &widgets.category_games_button,
            active == Some(SpotlightCategory::Games),
        );
        set_toggle_button_state(
            &widgets.category_graphics_button,
            active == Some(SpotlightCategory::Graphics),
        );
        set_toggle_button_state(
            &widgets.category_music_button,
            active == Some(SpotlightCategory::Music),
        );
        set_toggle_button_state(
            &widgets.category_productivity_button,
            active == Some(SpotlightCategory::Productivity),
        );
        set_toggle_button_state(
            &widgets.category_utilities_button,
            active == Some(SpotlightCategory::Utilities),
        );
        set_toggle_button_state(
            &widgets.category_video_button,
            active == Some(SpotlightCategory::Video),
        );
    }

    fn update_spotlight_installed_flags(self: &Rc<Self>) {
        let installed = {
            let state = self.state.borrow();
            state.installed_set.clone()
        };

        let mut needs_rebuild = false;

        {
            let mut state = self.state.borrow_mut();
            for pkg in &mut state.spotlight_recent {
                pkg.installed = installed.contains(&pkg.name);
            }
            for entries in state.spotlight_categories.values_mut() {
                for pkg in entries {
                    pkg.installed = installed.contains(&pkg.name);
                }
            }
            if let Some(backup) = state.spotlight_search_backup.as_mut() {
                for pkg in backup.iter_mut() {
                    pkg.installed = installed.contains(&pkg.name);
                }
            }
            if state.active_spotlight_category.is_some() {
                for pkg in &mut state.search_results {
                    pkg.installed = installed.contains(&pkg.name);
                }
                needs_rebuild = true;
            }
        }

        if needs_rebuild {
            self.rebuild_search_list();
        }
    }

    fn apply_spotlight_category(self: &Rc<Self>, category: SpotlightCategory, store_backup: bool) {
        let status_snapshot = if store_backup {
            Some(self.widgets.discover.status_label.text().to_string())
        } else {
            None
        };

        let packages = {
            let state = self.state.borrow();
            state
                .spotlight_categories
                .get(&category)
                .cloned()
                .unwrap_or_default()
        };

        {
            let mut state = self.state.borrow_mut();
            if store_backup && state.spotlight_search_backup.is_none() {
                state.spotlight_search_backup = Some(state.search_results.clone());
                state.spotlight_status_backup = status_snapshot;
            }
            state.active_spotlight_category = Some(category);
            state.search_results = packages;
            state.selected_search = None;
            state.discover_mode = DiscoverMode::Spotlight;
            state.discover_detail_focus = None;
        }

        self.set_category_button_state(Some(category));

        let count = {
            let state = self.state.borrow();
            state.search_results.len()
        };

        let label = if count == 0 {
            format!(
                "No spotlight picks found for {}.",
                category_display_name(category)
            )
        } else {
            format!(
                "Showing {} {} spotlight pick{}.",
                count,
                category_display_name(category).to_lowercase(),
                if count == 1 { "" } else { "s" }
            )
        };

        self.rebuild_search_list();
        self.clear_discover_details(false);
        self.set_discover_status(Some(&label));
        self.update_discover_layout();
    }

    fn clear_spotlight_category(self: &Rc<Self>) {
        let (backup_results, backup_status) = {
            let mut state = self.state.borrow_mut();
            state.active_spotlight_category = None;
            (
                state.spotlight_search_backup.take(),
                state.spotlight_status_backup.take(),
            )
        };

        self.set_category_button_state(None);

        if let Some(results) = backup_results {
            {
                let mut state = self.state.borrow_mut();
                state.search_results = results;
                state.selected_search = None;
                state.discover_mode = DiscoverMode::Spotlight;
                state.discover_detail_focus = None;
            }
            self.rebuild_search_list();
            self.clear_discover_details(false);
            if let Some(status) = backup_status {
                self.set_discover_status(Some(&status));
            }
        } else {
            {
                let mut state = self.state.borrow_mut();
                state.search_results.clear();
                state.selected_search = None;
                state.discover_mode = DiscoverMode::Spotlight;
                state.discover_detail_cache.clear();
                state.discover_detail_loading.clear();
                state.discover_detail_errors.clear();
                state.discover_detail_focus = None;
            }
            self.rebuild_search_list();
            self.clear_discover_details(false);
        }

        self.update_discover_layout();
    }

    fn refresh_active_spotlight_category(self: &Rc<Self>) {
        let category = {
            let state = self.state.borrow();
            state.active_spotlight_category
        };

        if let Some(category) = category {
            self.apply_spotlight_category(category, false);
        }
    }

    fn handle_spotlight_category_toggle(
        self: &Rc<Self>,
        category: SpotlightCategory,
        active: bool,
    ) {
        if active {
            self.apply_spotlight_category(category, true);
            self.run_category_search(category);
        } else {
            let should_clear = {
                let state = self.state.borrow();
                state.active_spotlight_category == Some(category)
            };
            if should_clear {
                self.clear_spotlight_category();
            }
        }
    }

    fn run_category_search(self: &Rc<Self>, category: SpotlightCategory) {
        let query = match category {
            SpotlightCategory::Browsers => "web browser",
            SpotlightCategory::Chat => "chat",
            SpotlightCategory::Games => "game",
            SpotlightCategory::Email => "email",
            SpotlightCategory::Graphics => "graphics",
            SpotlightCategory::Music => "music",
            SpotlightCategory::Productivity => "productivity",
            SpotlightCategory::Utilities => "utility",
            SpotlightCategory::Video => "video",
        };

        self.widgets.discover.search_entry.set_text(query);
        self.on_search_requested();
    }

    fn on_spotlight_recent_selected(self: &Rc<Self>, row: Option<gtk::ListBoxRow>) {
        let Some(row) = row else {
            return;
        };
        self.activate_spotlight_recent_row(&row);
    }

    fn on_spotlight_row_activated(self: &Rc<Self>, row: &gtk::ListBoxRow) {
        self.activate_spotlight_recent_row(row);
    }

    fn activate_spotlight_recent_row(self: &Rc<Self>, row: &gtk::ListBoxRow) {
        let name = row
            .child()
            .and_then(|child| child.downcast::<adw::ActionRow>().ok())
            .map(|action_row| action_row.title().to_string())
            .unwrap_or_default();

        let index = row.index();
        if index < 0 {
            if !name.is_empty() {
                self.widgets.discover.search_entry.set_text(&name);
            }
            self.on_search_requested();
            return;
        }
        let pkg = {
            let state = self.state.borrow();
            state.spotlight_recent.get(index as usize).cloned()
        };

        if let Some(pkg) = pkg {
            {
                let mut state = self.state.borrow_mut();
                state.spotlight_recent_selected = Some(pkg.name.clone());
                state.discover_detail_focus = Some(pkg.clone());
                state.discover_detail_package = Some(pkg.name.clone());
                state.discover_detail_navigation_active = false;
                state.discover_detail_history.clear();
                state.pending_discover_target = None;
            }
            self.request_discover_detail(&pkg.name);
            self.update_spotlight_recent_detail();
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("list");
            self.widgets
                .discover
                .spotlight_recent_scroller
                .set_visible(true);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_reveal_child(true);
            self.widgets
                .discover
                .spotlight_recent_detail_container
                .set_visible(true);
            self.update_discover_details();
            return;
        }

        self.on_search_requested();
    }

    fn on_spotlight_recent_back(self: &Rc<Self>) {
        self.clear_spotlight_recent_selection();
    }

    fn clear_spotlight_recent_selection(self: &Rc<Self>) {
        {
            let mut state = self.state.borrow_mut();
            if state.spotlight_recent_selected.take().is_some() {
                state.discover_detail_focus = None;
                state.discover_detail_package = None;
                state.discover_detail_navigation_active = false;
                state.discover_detail_history.clear();
                state.pending_discover_target = None;
            }
        }

        self.widgets.discover.spotlight_recent_list.unselect_all();
        self.widgets.discover.spotlight_recent_detail_spinner.stop();
        self.widgets
            .discover
            .spotlight_recent_detail_spinner
            .set_visible(false);

        self.widgets
            .discover
            .spotlight_recent_detail_revealer
            .set_reveal_child(false);
        self.widgets
            .discover
            .spotlight_recent_detail_container
            .set_visible(false);

        let recent_items = {
            let state = self.state.borrow();
            state.spotlight_recent.clone()
        };
        populate_spotlight_list(&self.widgets.discover.spotlight_recent_list, &recent_items);

        if recent_items.is_empty() {
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("placeholder");
        } else {
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("list");
        }

        self.update_spotlight_recent_detail();
        self.update_discover_details();
    }

    fn update_spotlight_views(self: &Rc<Self>) {
        let (recent, loading, last_refresh, active_category, selected_recent) = {
            let state = self.state.borrow();
            (
                state.spotlight_recent.clone(),
                state.spotlight_loading,
                state.spotlight_last_refresh,
                state.active_spotlight_category,
                state.spotlight_recent_selected.clone(),
            )
        };

        self.set_category_button_state(active_category);

        let spinner = &self.widgets.discover.spotlight_spinner;
        let status_label = &self.widgets.discover.spotlight_status;

        self.widgets
            .discover
            .spotlight_refresh_button
            .set_sensitive(!loading);

        if loading {
            spinner.set_visible(true);
            spinner.start();
            status_label.set_text("Refreshing spotlight…");
        } else {
            spinner.stop();
            spinner.set_visible(false);
            let status_text = if let Some(last) = last_refresh {
                format!("Last updated {}.", format_relative_time(last))
            } else if recent.is_empty() {
                "Last updated —".to_string()
            } else {
                "Last updated just now.".to_string()
            };
            status_label.set_text(&status_text);
        }

        populate_spotlight_list(&self.widgets.discover.spotlight_recent_list, &recent);
        let detail_available = selected_recent
            .as_ref()
            .map(|name| recent.iter().any(|pkg| pkg.name == *name))
            .unwrap_or(false);

        if selected_recent.is_some() && !detail_available {
            let mut state = self.state.borrow_mut();
            state.spotlight_recent_selected = None;
        }

        if detail_available {
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("list");
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_reveal_child(true);
            self.widgets
                .discover
                .spotlight_recent_detail_container
                .set_visible(true);
            self.update_spotlight_recent_detail();
        } else if recent.is_empty() {
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("placeholder");
            self.widgets
                .discover
                .spotlight_recent_scroller
                .set_visible(false);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_reveal_child(false);
            self.widgets
                .discover
                .spotlight_recent_detail_container
                .set_visible(false);
        } else {
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("list");
            self.widgets
                .discover
                .spotlight_recent_scroller
                .set_visible(true);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_reveal_child(false);
            self.widgets
                .discover
                .spotlight_recent_detail_container
                .set_visible(false);
        }
    }

    fn update_spotlight_recent_detail(self: &Rc<Self>) {
        let (pkg, detail, loading, error, install_in_progress, remove_in_progress) = {
            let state = self.state.borrow();
            let selected = state.spotlight_recent_selected.clone();
            let pkg = selected.as_ref().and_then(|name| {
                state
                    .spotlight_recent
                    .iter()
                    .find(|pkg| &pkg.name == name)
                    .cloned()
            });
            let detail = selected
                .as_ref()
                .and_then(|name| state.discover_detail_cache.get(name).cloned());
            let loading = selected
                .as_ref()
                .map_or(false, |name| state.discover_detail_loading.contains(name));
            let error = selected
                .as_ref()
                .and_then(|name| state.discover_detail_errors.get(name).cloned());
            (
                pkg,
                detail,
                loading,
                error,
                state.install_in_progress,
                state.remove_in_progress,
            )
        };

        let widgets = &self.widgets.discover;
        let back_button = &widgets.spotlight_recent_back_button;
        let spinner = &widgets.spotlight_recent_detail_spinner;
        let status_label = &widgets.spotlight_recent_detail_status;
        let update_label = &widgets.spotlight_recent_detail_update_label;
        let version_value = &widgets.spotlight_recent_detail_version_value;
        let download_value = &widgets.spotlight_recent_detail_download_value;
        let repo_row = &widgets.spotlight_recent_detail_repo_row;
        let repo_value = &widgets.spotlight_recent_detail_repo_value;
        let homepage_row = &widgets.spotlight_recent_detail_homepage_row;
        let homepage_link = &widgets.spotlight_recent_detail_homepage_link;
        let maintainer_row = &widgets.spotlight_recent_detail_maintainer_row;
        let maintainer_value = &widgets.spotlight_recent_detail_maintainer_value;
        let license_row = &widgets.spotlight_recent_detail_license_row;
        let license_value = &widgets.spotlight_recent_detail_license_value;
        let updated_row = &widgets.spotlight_recent_detail_updated_row;
        let updated_value = &widgets.spotlight_recent_detail_updated_value;
        let description_label = &widgets.spotlight_recent_detail_description;
        let dependencies_stack = &widgets.spotlight_recent_detail_dependencies_stack;
        let dependencies_list = &widgets.spotlight_recent_detail_dependencies_list;
        let dependencies_placeholder = &widgets.spotlight_recent_detail_dependencies_placeholder;
        let action_button = &widgets.spotlight_recent_action_button;

        if let Some(pkg) = pkg {
            back_button.set_visible(true);
            widgets.spotlight_recent_detail_container.set_visible(true);
            widgets.spotlight_recent_detail_name.set_text(&pkg.name);

            let actions_enabled = !loading && !install_in_progress && !remove_in_progress;
            action_button.set_visible(true);
            action_button.set_sensitive(actions_enabled);
            action_button.remove_css_class("suggested-action");
            action_button.remove_css_class("destructive-action");
            if pkg.installed {
                action_button.set_label("Remove");
                action_button.add_css_class("destructive-action");
            } else {
                action_button.set_label("Install");
                action_button.add_css_class("suggested-action");
            }

            if loading {
                spinner.set_visible(true);
                spinner.start();
                status_label.set_visible(true);
                status_label.set_text("Loading details…");
            } else {
                spinner.stop();
                spinner.set_visible(false);
                if let Some(error) = error.clone() {
                    status_label.set_visible(true);
                    status_label.set_text(&format!("Could not load additional details: {}", error));
                } else {
                    status_label.set_visible(false);
                    status_label.set_text("");
                }
            }

            let version_text = detail
                .as_ref()
                .and_then(|d| d.version.clone())
                .filter(|v| !v.is_empty())
                .or_else(|| (!pkg.version.is_empty()).then(|| pkg.version.clone()))
                .unwrap_or_else(|| "—".to_string());
            version_value.set_text(&version_text);

            set_download_label(
                download_value,
                detail.as_ref().and_then(|d| d.download_bytes),
                detail.as_ref().and_then(|d| d.download.as_deref()),
                pkg.download_bytes,
                pkg.download_size.as_deref(),
            );

            let repo_text = detail
                .as_ref()
                .and_then(|d| d.repository.clone())
                .or_else(|| pkg.repository.clone());
            if let Some(repo) = repo_text {
                repo_row.set_visible(true);
                repo_value.set_visible(true);
                repo_value.set_text(&repo);
            } else {
                repo_row.set_visible(false);
                repo_value.set_visible(false);
                repo_value.set_text("");
            }

            if let Some(detail) = detail.clone() {
                if let Some(homepage) = detail.homepage {
                    homepage_row.set_visible(true);
                    homepage_link.set_visible(true);
                    homepage_link.set_uri(&homepage);
                    homepage_link.set_label(&homepage);
                    homepage_link.set_tooltip_text(Some(&homepage));
                } else {
                    homepage_row.set_visible(false);
                    homepage_link.set_visible(false);
                    homepage_link.set_label("");
                    homepage_link.set_uri("");
                    homepage_link.set_tooltip_text(None);
                }

                if let Some(maintainer) = detail.maintainer {
                    let friendly = sanitize_contact_field(&maintainer);
                    if friendly.is_empty() {
                        maintainer_row.set_visible(false);
                        maintainer_value.set_visible(false);
                        maintainer_value.set_text("");
                    } else {
                        maintainer_row.set_visible(true);
                        maintainer_value.set_visible(true);
                        maintainer_value.set_text(&friendly);
                    }
                } else {
                    maintainer_row.set_visible(false);
                    maintainer_value.set_visible(false);
                    maintainer_value.set_text("");
                }

                if let Some(license) = detail.license {
                    license_row.set_visible(true);
                    license_value.set_visible(true);
                    license_value.set_text(&license);
                } else {
                    license_row.set_visible(false);
                    license_value.set_visible(false);
                    license_value.set_text("");
                }

                if let Some(updated) = pkg.build_date {
                    updated_row.set_visible(true);
                    updated_value.set_visible(true);
                    updated_value.set_text(&format_relative_time(updated));
                } else {
                    updated_row.set_visible(false);
                    updated_value.set_visible(false);
                    updated_value.set_text("");
                }

                if pkg.installed {
                    update_label.set_text("Installed on this system.");
                    update_label.set_visible(true);
                } else {
                    update_label.set_visible(false);
                    update_label.set_text("");
                }

                let description = detail
                    .description
                    .clone()
                    .unwrap_or_else(|| pkg.description.clone());
                if description.trim().is_empty() {
                    description_label.set_text("This package does not provide a description.");
                } else {
                    description_label.set_text(&description);
                }

                clear_listbox(dependencies_list);
                let installed_set = {
                    let state = self.state.borrow();
                    state.installed_set.clone()
                };
                if detail.dependencies.is_empty() {
                    dependencies_placeholder.set_text("No runtime dependencies.");
                    dependencies_list.set_visible(false);
                    dependencies_stack.set_visible_child_name("placeholder");
                } else {
                    for dependency in &detail.dependencies {
                        let row = adw::ActionRow::builder()
                            .title(dependency.name.as_str())
                            .build();
                        row.set_activatable(true);
                        row.set_focusable(true);

                        let status_text = if installed_set.contains(&dependency.name) {
                            "Installed"
                        } else {
                            "Not installed"
                        };
                        let status_label = gtk::Label::builder()
                            .label(status_text)
                            .halign(gtk::Align::End)
                            .valign(gtk::Align::Center)
                            .build();
                        status_label.add_css_class("dim-label");
                        row.add_suffix(&status_label);

                        let package_name = dependency.name.clone();
                        row.connect_activated(
                            glib::clone!(@strong self as controller => move |_| {
                                controller.on_discover_dependency_clicked(package_name.clone());
                            }),
                        );

                        dependencies_list.append(&row);
                    }
                    dependencies_list.set_visible(true);
                    dependencies_stack.set_visible_child_name("list");
                }
            } else if loading {
                homepage_row.set_visible(false);
                homepage_link.set_visible(false);
                homepage_link.set_label("");
                homepage_link.set_uri("");
                homepage_link.set_tooltip_text(None);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                maintainer_value.set_text("");
                license_row.set_visible(false);
                license_value.set_visible(false);
                license_value.set_text("");
                updated_row.set_visible(false);
                updated_value.set_visible(false);
                updated_value.set_text("");
                update_label.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or(detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Loading dependencies…");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description_label.set_text("Loading package details…");
            } else if let Some(err) = error.clone() {
                homepage_row.set_visible(false);
                homepage_link.set_visible(false);
                homepage_link.set_label("");
                homepage_link.set_uri("");
                homepage_link.set_tooltip_text(None);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                maintainer_value.set_text("");
                license_row.set_visible(false);
                license_value.set_visible(false);
                license_value.set_text("");
                updated_row.set_visible(false);
                updated_value.set_visible(false);
                updated_value.set_text("");
                update_label.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or(detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Dependency information unavailable.");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description_label.set_text(&format!("Could not load package details: {}", err));
            } else {
                homepage_row.set_visible(false);
                homepage_link.set_visible(false);
                homepage_link.set_label("");
                homepage_link.set_uri("");
                homepage_link.set_tooltip_text(None);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                maintainer_value.set_text("");
                license_row.set_visible(false);
                license_value.set_visible(false);
                license_value.set_text("");
                updated_row.set_visible(false);
                updated_value.set_visible(false);
                updated_value.set_text("");
                update_label.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or(detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Loading dependencies…");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description_label.set_text("Loading package details…");
                self.request_discover_detail(&pkg.name);
            }
        } else {
            back_button.set_visible(false);
            widgets.spotlight_recent_detail_container.set_visible(false);
            spinner.stop();
            spinner.set_visible(false);
            status_label.set_visible(false);
            status_label.set_text("");
            widgets
                .spotlight_recent_detail_name
                .set_text("Select a package");
            version_value.set_text("—");
            download_value.set_text("—");
            repo_row.set_visible(false);
            repo_value.set_visible(false);
            repo_value.set_text("");
            homepage_row.set_visible(false);
            homepage_link.set_visible(false);
            homepage_link.set_label("");
            homepage_link.set_uri("");
            homepage_link.set_tooltip_text(None);
            maintainer_row.set_visible(false);
            maintainer_value.set_visible(false);
            maintainer_value.set_text("");
            license_row.set_visible(false);
            license_value.set_visible(false);
            license_value.set_text("");
            updated_row.set_visible(false);
            updated_value.set_visible(false);
            updated_value.set_text("");
            update_label.set_visible(false);
            description_label.set_text("Select a package to see details.");
            clear_listbox(dependencies_list);
            dependencies_placeholder.set_text("No runtime dependencies.");
            dependencies_list.set_visible(false);
            dependencies_stack.set_visible_child_name("placeholder");
            action_button.set_visible(false);
            action_button.remove_css_class("suggested-action");
            action_button.remove_css_class("destructive-action");
        }
    }

    fn update_discover_layout(&self) {
        let (mode, has_results) = {
            let state = self.state.borrow();
            (state.discover_mode, !state.search_results.is_empty())
        };

        let spotlight_visible = mode == DiscoverMode::Spotlight;
        self.widgets
            .discover
            .spotlight_section_box
            .set_visible(spotlight_visible);
        self.widgets.discover.scroller.set_visible(has_results);
        self.widgets.discover.scroller.set_vexpand(has_results);
        self.widgets.discover.content_row.set_visible(has_results);
        self.widgets.discover.content_row.set_vexpand(has_results);
    }

    fn update_updates_badge(&self) {
        let count = self.state.borrow().available_updates.len();
        if count > 0 {
            self.widgets.updates_badge.set_visible(true);
            self.widgets.updates_badge.set_text(&count.to_string());
        } else {
            self.widgets.updates_badge.set_visible(false);
        }
        self.widgets.updates_page.set_badge_number(count as u32);
    }

    fn clear_search_results(self: &Rc<Self>) {
        let mut state = self.state.borrow_mut();
        state.search_results.clear();
        state.selected_search = None;
        state.discover_mode = DiscoverMode::Spotlight;
        state.discover_detail_cache.clear();
        state.discover_detail_loading.clear();
        state.discover_detail_errors.clear();
        state.discover_detail_focus = None;
        drop(state);
        self.rebuild_search_list();
        self.clear_discover_details(false);
        self.update_discover_layout();
    }

    fn clear_installed_results(self: &Rc<Self>) {
        let mut state = self.state.borrow_mut();
        state.installed_packages.clear();
        state.installed_set.clear();
        state.installed_filtered.clear();
        state.installed_selected.clear();
        state.selected_installed = None;
        state.installed_last_refresh = None;
        state.installed_detail_package = None;
        state.installed_detail_history.clear();
        state.installed_detail_navigation_active = false;
        drop(state);
        self.rebuild_installed_list();
        self.update_installed_selection_ui();
        self.update_installed_summary();
        self.update_installed_details();
        self.update_installed_detail_back_button();
    }

    fn update_search_installed_flags(self: &Rc<Self>) {
        {
            let state = self.state.borrow();
            if state.search_results.is_empty() {
                return;
            }
        }

        {
            let installed_set = self.state.borrow().installed_set.clone();
            let mut state = self.state.borrow_mut();
            for pkg in &mut state.search_results {
                pkg.installed = installed_set.contains(&pkg.name);
            }
        }
        self.rebuild_search_list();
    }

    fn update_discover_details(self: &Rc<Self>) {
        let stack = &self.widgets.discover.detail_stack;
        let button = &self.widgets.discover.detail_action_button;
        let version_value = &self.widgets.discover.detail_version_value;
        let download_value = &self.widgets.discover.detail_download_value;
        let homepage_row = &self.widgets.discover.detail_homepage_row;
        let homepage_link = &self.widgets.discover.detail_homepage_link;
        let maintainer_row = &self.widgets.discover.detail_maintainer_row;
        let maintainer_value = &self.widgets.discover.detail_maintainer_value;
        let license_row = &self.widgets.discover.detail_license_row;
        let license_value = &self.widgets.discover.detail_license_value;
        let update_label = &self.widgets.discover.detail_update_label;
        let repo_row = &self.widgets.discover.detail_repository_row;
        let repo_value = &self.widgets.discover.detail_repository_value;
        let description_label = &self.widgets.discover.detail_description;
        let dependencies_stack = &self.widgets.discover.detail_dependencies_stack;
        let dependencies_list = &self.widgets.discover.detail_dependencies_list;
        let dependencies_placeholder = &self.widgets.discover.detail_dependencies_placeholder;

        if let Some(pkg) = self.current_search_selection() {
            self.set_discover_row_buttons_visible(false);
            self.widgets
                .discover
                .detail_name
                .set_text(pkg.name.as_str());
            self.widgets.discover.detail_frame.set_visible(true);
            self.widgets.discover.detail_close_button.set_visible(true);
            self.widgets
                .discover
                .detail_close_button
                .set_sensitive(true);
            stack.set_visible_child_name("detail");
            {
                let mut state = self.state.borrow_mut();
                state.discover_detail_package = Some(pkg.name.clone());
                state.pending_discover_target = None;
            }

            if pkg.version.is_empty() {
                version_value.set_text("—");
            } else {
                version_value.set_text(pkg.version.as_str());
            }

            let (detail, loading, error, install_in_progress, remove_in_progress) = {
                let state = self.state.borrow();
                (
                    state.discover_detail_cache.get(&pkg.name).cloned(),
                    state.discover_detail_loading.contains(&pkg.name),
                    state.discover_detail_errors.get(&pkg.name).cloned(),
                    state.install_in_progress,
                    state.remove_in_progress,
                )
            };

            if let Some(detail) = detail {
                if pkg.version.is_empty() {
                    if let Some(ver) = detail.version.clone() {
                        version_value.set_text(&ver);
                    } else {
                        version_value.set_text("—");
                    }
                }

                let description = detail
                    .description
                    .clone()
                    .unwrap_or_else(|| pkg.description.clone());
                if description.is_empty() {
                    description_label.set_text("This package does not provide a description.");
                } else {
                    description_label.set_text(&description);
                }

                if let Some(repo) = detail.repository.or_else(|| pkg.repository.clone()) {
                    repo_row.set_visible(true);
                    repo_value.set_visible(true);
                    repo_value.set_text(repo.as_str());
                } else {
                    repo_row.set_visible(false);
                    repo_value.set_visible(false);
                    repo_value.set_text("");
                }

                set_download_label(
                    download_value,
                    detail.download_bytes,
                    detail.download.as_deref(),
                    pkg.download_bytes,
                    pkg.download_size.as_deref(),
                );

                if let Some(homepage) = detail.homepage {
                    homepage_row.set_visible(true);
                    homepage_link.set_visible(true);
                    homepage_link.set_uri(&homepage);
                    homepage_link.set_label(&homepage);
                    homepage_link.set_tooltip_text(Some(&homepage));
                } else {
                    homepage_row.set_visible(false);
                    homepage_link.set_visible(false);
                    homepage_link.set_label("");
                    homepage_link.set_uri("");
                }

                if let Some(maintainer) = detail.maintainer {
                    let friendly = sanitize_contact_field(&maintainer);
                    if friendly.is_empty() {
                        maintainer_row.set_visible(false);
                        maintainer_value.set_visible(false);
                        maintainer_value.set_text("");
                    } else {
                        maintainer_row.set_visible(true);
                        maintainer_value.set_visible(true);
                        maintainer_value.set_text(&friendly);
                    }
                } else {
                    maintainer_row.set_visible(false);
                    maintainer_value.set_visible(false);
                    maintainer_value.set_text("");
                }

                if let Some(license) = detail.license {
                    license_row.set_visible(true);
                    license_value.set_visible(true);
                    license_value.set_text(&license);
                } else {
                    license_row.set_visible(false);
                    license_value.set_visible(false);
                    license_value.set_text("");
                }

                update_label.set_visible(false);

                clear_listbox(dependencies_list);
                let installed_set = {
                    let state = self.state.borrow();
                    state.installed_set.clone()
                };

                if detail.dependencies.is_empty() {
                    dependencies_placeholder.set_text("No runtime dependencies.");
                    dependencies_list.set_visible(false);
                    dependencies_stack.set_visible_child_name("placeholder");
                } else {
                    for dependency in &detail.dependencies {
                        let row = adw::ActionRow::builder()
                            .title(dependency.name.as_str())
                            .build();
                        row.set_activatable(true);
                        row.set_focusable(true);

                        let status_text = if installed_set.contains(&dependency.name) {
                            "Installed"
                        } else {
                            "Not installed"
                        };
                        let status_label = gtk::Label::builder()
                            .label(status_text)
                            .halign(gtk::Align::End)
                            .valign(gtk::Align::Center)
                            .build();
                        status_label.add_css_class("dim-label");
                        row.add_suffix(&status_label);

                        let package_name = dependency.name.clone();
                        row.connect_activated(
                            glib::clone!(@strong self as controller => move |_| {
                                controller.on_discover_dependency_clicked(package_name.clone());
                            }),
                        );

                        dependencies_list.append(&row);
                    }
                    dependencies_list.set_visible(true);
                    dependencies_stack.set_visible_child_name("list");
                }
            } else if loading {
                repo_row.set_visible(false);
                repo_value.set_visible(false);
                homepage_row.set_visible(false);
                homepage_link.set_visible(false);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                license_row.set_visible(false);
                license_value.set_visible(false);
                update_label.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or(detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Loading dependencies…");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description_label.set_text("Loading package details…");
            } else if let Some(err) = error {
                repo_row.set_visible(false);
                repo_value.set_visible(false);
                homepage_row.set_visible(false);
                homepage_link.set_visible(false);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                license_row.set_visible(false);
                license_value.set_visible(false);
                update_label.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or(detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Dependency information unavailable.");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description_label.set_text(&format!("Could not load package details: {}", err));
            } else {
                repo_row.set_visible(false);
                repo_value.set_visible(false);
                homepage_row.set_visible(false);
                homepage_link.set_visible(false);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                license_row.set_visible(false);
                license_value.set_visible(false);
                update_label.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or(detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Loading dependencies…");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description_label.set_text("Loading package details…");
                self.request_discover_detail(&pkg.name);
            }

            button.set_visible(true);
            button.remove_css_class("suggested-action");
            button.remove_css_class("destructive-action");

            if pkg.installed {
                button.set_label("Remove");
                button.add_css_class("destructive-action");
                button.set_sensitive(!remove_in_progress);
                button.set_tooltip_text(Some("Remove this package."));
            } else {
                button.set_label("Install");
                button.add_css_class("suggested-action");
                button.set_sensitive(!install_in_progress);
                button.set_tooltip_text(Some("Install this package."));
            }
        } else {
            self.clear_discover_details(true);
        }

        self.update_discover_detail_back_button();
    }

    fn request_discover_detail(self: &Rc<Self>, package: &str) {
        let package_name = package.to_string();
        {
            let mut state = self.state.borrow_mut();
            if state.discover_detail_cache.contains_key(&package_name)
                || state.discover_detail_loading.contains(&package_name)
            {
                return;
            }
            state.discover_detail_errors.remove(&package_name);
            state.discover_detail_loading.insert(package_name.clone());
        }

        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = query_discover_detail(&package_name);
            let _ = sender.send(AppMessage::DiscoverDetailLoaded {
                package: package_name,
                result,
            });
        });
    }

    fn flag_installed_state(self: &Rc<Self>, package_name: &str, installed: bool) {
        {
            let mut state = self.state.borrow_mut();
            if installed {
                state.installed_set.insert(package_name.to_string());
            } else {
                state.installed_set.remove(package_name);
            }

            for pkg in &mut state.search_results {
                if pkg.name == package_name {
                    pkg.installed = installed;
                }
            }

            if let Some(focus) = state.discover_detail_focus.as_mut() {
                if focus.name == package_name {
                    focus.installed = installed;
                }
            }

            if !installed {
                state
                    .installed_packages
                    .retain(|pkg| pkg.name != package_name);
            }
        }

        self.rebuild_search_list();
        self.rebuild_installed_list();
        self.update_spotlight_installed_flags();
        self.update_spotlight_views();
        self.refresh_active_spotlight_category();
        self.update_discover_details();
    }

    fn clear_discover_details(&self, preserve_navigation: bool) {
        self.set_discover_status(None);
        {
            let mut state = self.state.borrow_mut();
            if !preserve_navigation {
                state.discover_detail_history.clear();
                state.discover_detail_navigation_active = false;
                state.pending_discover_target = None;
            }
            state.discover_detail_package = None;
            state.discover_detail_focus = None;
        }
        self.widgets
            .discover
            .detail_name
            .set_text("Select a package");
        self.widgets
            .discover
            .detail_stack
            .set_visible_child_name("placeholder");
        self.widgets.discover.detail_frame.set_visible(false);
        self.widgets.discover.detail_close_button.set_visible(false);
        self.widgets
            .discover
            .detail_close_button
            .set_sensitive(false);
        self.widgets
            .discover
            .detail_action_button
            .set_visible(false);
        self.widgets
            .discover
            .detail_homepage_link
            .set_visible(false);
        self.widgets
            .discover
            .detail_maintainer_row
            .set_visible(false);
        self.widgets
            .discover
            .detail_maintainer_value
            .set_visible(false);
        self.widgets
            .discover
            .detail_license_value
            .set_visible(false);
        self.widgets
            .discover
            .detail_repository_row
            .set_visible(false);
        self.widgets
            .discover
            .detail_repository_value
            .set_visible(false);
        self.widgets
            .discover
            .detail_description
            .set_text("Select a package to see details.");
        clear_listbox(&self.widgets.discover.detail_dependencies_list);
        self.widgets
            .discover
            .detail_dependencies_placeholder
            .set_text("No runtime dependencies.");
        self.widgets
            .discover
            .detail_dependencies_list
            .set_visible(false);
        self.widgets
            .discover
            .detail_dependencies_stack
            .set_visible_child_name("placeholder");
        self.set_discover_row_buttons_visible(true);
        self.update_discover_detail_back_button();
    }

    fn current_search_selection(&self) -> Option<PackageInfo> {
        let state = self.state.borrow();
        if let Some(idx) = state.selected_search {
            if let Some(pkg) = state.search_results.get(idx) {
                return Some(pkg.clone());
            }
        }
        state.discover_detail_focus.clone()
    }

    fn select_search_row_by_name(self: &Rc<Self>, name: &str) {
        let _ = self.focus_discover_package(name, false);
    }

    fn on_discover_detail_close(self: &Rc<Self>) {
        self.widgets.discover.list.unselect_all();
        self.clear_discover_details(false);
    }
}

fn clear_listbox(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn set_toggle_button_state(button: &gtk::ToggleButton, active: bool) {
    if button.is_active() != active {
        button.set_active(active);
    }
}

fn populate_spotlight_list(list: &gtk::ListBox, packages: &[PackageInfo]) {
    clear_listbox(list);
    for pkg in packages {
        let row = build_package_row(pkg);
        list.append(&row);
    }
}

fn set_download_label(
    label: &gtk::Label,
    detail_bytes: Option<u64>,
    detail_text: Option<&str>,
    pkg_bytes: Option<u64>,
    pkg_text: Option<&str>,
) {
    if let Some(text) = detail_text {
        label.set_text(text);
    } else if let Some(bytes) = detail_bytes.or(pkg_bytes) {
        label.set_text(&format_size(bytes));
    } else if let Some(text) = pkg_text {
        label.set_text(text);
    } else {
        label.set_text("—");
    }
}

fn build_category_button(icon_name: &str, label: &str) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::builder().build();
    button.add_css_class("pill");
    button.add_css_class("flat");
    button.set_hexpand(false);
    button.set_halign(gtk::Align::Fill);
    button.set_margin_top(4);
    button.set_margin_bottom(4);
    button.set_margin_start(0);
    button.set_margin_end(0);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(5)
        .halign(gtk::Align::Start)
        .build();

    let icon = gtk::Image::builder()
        .resource(icon_name)
        .pixel_size(16)
        .build();
    icon.add_css_class("dim-label");

    let text = gtk::Label::builder()
        .label(label)
        .halign(gtk::Align::Center)
        .build();
    text.add_css_class("title-4");

    content.append(&icon);
    content.append(&text);
    button.set_child(Some(&content));

    button
}

fn build_discover_page() -> (gtk::Box, DiscoverWidgets) {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    container.set_vexpand(true);

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search the Void Linux repositories")
        .hexpand(true)
        .build();

    let search_button = gtk::Button::builder().label("Search").build();
    search_button.add_css_class("suggested-action");

    let search_spinner = gtk::Spinner::new();
    search_spinner.set_visible(false);
    search_spinner.set_valign(gtk::Align::Center);

    let search_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    search_row.append(&search_entry);
    search_row.append(&search_button);
    search_row.append(&search_spinner);

    let categories_list = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .halign(gtk::Align::Start)
        .build();
    categories_list.set_valign(gtk::Align::Start);
    categories_list.set_hexpand(false);

    let category_browsers_button =
        build_category_button("/tech/geektoshi/Nebula/icons/browsers.svg", "Browsers");
    let category_chat_button =
        build_category_button("/tech/geektoshi/Nebula/icons/chat.svg", "Chat");
    let category_email_button =
        build_category_button("/tech/geektoshi/Nebula/icons/email.svg", "E-mail");
    let category_games_button =
        build_category_button("/tech/geektoshi/Nebula/icons/games.svg", "Games");
    let category_graphics_button =
        build_category_button("/tech/geektoshi/Nebula/icons/graphics.svg", "Graphics");
    let category_productivity_button = build_category_button(
        "/tech/geektoshi/Nebula/icons/productivity.svg",
        "Productivity",
    );
    let category_music_button =
        build_category_button("/tech/geektoshi/Nebula/icons/music.svg", "Music");
    let category_utilities_button =
        build_category_button("/tech/geektoshi/Nebula/icons/utilities.svg", "Utilities");
    let category_video_button =
        build_category_button("/tech/geektoshi/Nebula/icons/video.svg", "Video");

    category_chat_button.set_group(Some(&category_browsers_button));
    category_email_button.set_group(Some(&category_browsers_button));
    category_games_button.set_group(Some(&category_browsers_button));
    category_graphics_button.set_group(Some(&category_browsers_button));
    category_productivity_button.set_group(Some(&category_browsers_button));
    category_music_button.set_group(Some(&category_browsers_button));
    category_utilities_button.set_group(Some(&category_browsers_button));
    category_video_button.set_group(Some(&category_browsers_button));

    categories_list.append(&category_browsers_button);
    categories_list.append(&category_chat_button);
    categories_list.append(&category_email_button);
    categories_list.append(&category_games_button);
    categories_list.append(&category_graphics_button);
    categories_list.append(&category_music_button);
    categories_list.append(&category_productivity_button);
    categories_list.append(&category_utilities_button);
    categories_list.append(&category_video_button);

    let spotlight_status_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Center)
        .build();
    spotlight_status_row.set_valign(gtk::Align::Center);
    spotlight_status_row.set_margin_top(0);
    spotlight_status_row.set_margin_bottom(0);
    spotlight_status_row.set_margin_start(0);
    spotlight_status_row.set_margin_end(0);

    let spotlight_spinner = gtk::Spinner::new();
    spotlight_spinner.set_visible(false);
    spotlight_spinner.set_valign(gtk::Align::Center);
    spotlight_spinner.set_size_request(16, 16);

    let spotlight_status = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .wrap(false)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    spotlight_status.set_valign(gtk::Align::Center);
    spotlight_status.add_css_class("dim-label");
    spotlight_status.set_text("Loading spotlight metadata…");
    spotlight_status.set_xalign(0.5);

    spotlight_status_row.append(&spotlight_spinner);
    spotlight_status_row.append(&spotlight_status);

    let spotlight_section_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    spotlight_section_box.set_margin_top(6);
    spotlight_section_box.add_css_class("nebula-card");

    let spotlight_recent_list = gtk::ListBox::new();
    spotlight_recent_list.add_css_class("boxed-list");
    spotlight_recent_list.set_selection_mode(gtk::SelectionMode::Single);
    spotlight_recent_list.set_activate_on_single_click(true);
    spotlight_recent_list.set_focusable(true);

    let spotlight_recent_scroller = gtk::ScrolledWindow::builder()
        .min_content_height(240)
        .propagate_natural_width(true)
        .build();
    spotlight_recent_scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    spotlight_recent_scroller.set_propagate_natural_height(false);
    spotlight_recent_scroller.set_hexpand(true);
    spotlight_recent_scroller.set_vexpand(true);
    spotlight_recent_scroller.set_child(Some(&spotlight_recent_list));

    let spotlight_recent_overlay = gtk::Overlay::new();
    spotlight_recent_overlay.set_child(Some(&spotlight_recent_scroller));

    let spotlight_recent_placeholder = adw::StatusPage::builder()
        .title("Nothing updated recently")
        .description("Packages updated in the past 7 days will appear here.")
        .build();

    let spotlight_recent_stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();
    spotlight_recent_stack.set_vexpand(true);
    spotlight_recent_stack.add_named(&spotlight_recent_placeholder, Some("placeholder"));
    spotlight_recent_stack.add_named(&spotlight_recent_overlay, Some("list"));
    let recent_detail_back_button = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .has_frame(false)
        .tooltip_text("Back to recently updated")
        .visible(false)
        .build();
    recent_detail_back_button.add_css_class("flat");
    recent_detail_back_button.set_focus_on_click(false);
    recent_detail_back_button.set_valign(gtk::Align::Center);

    let recent_detail_name = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    recent_detail_name.add_css_class("title-2");

    let recent_detail_action_button = gtk::Button::builder()
        .label("Install")
        .width_request(140)
        .visible(false)
        .build();
    recent_detail_action_button.add_css_class("suggested-action");
    recent_detail_action_button.set_halign(gtk::Align::Start);
    recent_detail_action_button.set_valign(gtk::Align::Center);

    let recent_detail_spinner = gtk::Spinner::new();
    recent_detail_spinner.set_visible(false);
    recent_detail_spinner.set_valign(gtk::Align::Center);

    let recent_detail_header = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    recent_detail_header.set_valign(gtk::Align::Center);
    recent_detail_header.append(&recent_detail_back_button);
    recent_detail_header.append(&recent_detail_name);
    let recent_detail_header_spacer = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .build();
    recent_detail_header.append(&recent_detail_header_spacer);
    recent_detail_header.append(&recent_detail_spinner);

    let make_recent_metadata_label = |text: &str| {
        let label = gtk::Label::builder()
            .label(text)
            .halign(gtk::Align::Start)
            .build();
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        label.set_width_chars(14);
        label
    };

    let recent_detail_metadata_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();

    let recent_detail_version_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    let recent_detail_version_title = make_recent_metadata_label("Version");
    let recent_detail_version_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    recent_detail_version_value.set_hexpand(true);
    recent_detail_version_value.set_xalign(0.0);
    recent_detail_version_value.set_text("—");
    recent_detail_version_row.append(&recent_detail_version_title);
    recent_detail_version_row.append(&recent_detail_version_value);
    recent_detail_metadata_box.append(&recent_detail_version_row);

    let recent_detail_download_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    let recent_detail_download_title = make_recent_metadata_label("Download size");
    let recent_detail_download_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    recent_detail_download_value.set_hexpand(true);
    recent_detail_download_value.set_xalign(0.0);
    recent_detail_download_value.set_text("—");
    recent_detail_download_row.append(&recent_detail_download_title);
    recent_detail_download_row.append(&recent_detail_download_value);
    recent_detail_metadata_box.append(&recent_detail_download_row);

    let recent_detail_repo_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let recent_detail_repo_title = make_recent_metadata_label("Repository");
    let recent_detail_repo_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .visible(false)
        .build();
    recent_detail_repo_value.set_hexpand(true);
    recent_detail_repo_value.set_xalign(0.0);
    recent_detail_repo_row.append(&recent_detail_repo_title);
    recent_detail_repo_row.append(&recent_detail_repo_value);
    recent_detail_metadata_box.append(&recent_detail_repo_row);

    let recent_detail_homepage_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let recent_detail_homepage_title = make_recent_metadata_label("Homepage");
    let recent_detail_homepage_link = gtk::LinkButton::builder()
        .label("")
        .has_frame(false)
        .visible(false)
        .build();
    recent_detail_homepage_link.set_halign(gtk::Align::Start);
    recent_detail_homepage_link.set_hexpand(true);
    recent_detail_homepage_row.append(&recent_detail_homepage_title);
    recent_detail_homepage_row.append(&recent_detail_homepage_link);
    recent_detail_metadata_box.append(&recent_detail_homepage_row);

    let recent_detail_maintainer_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let recent_detail_maintainer_title = make_recent_metadata_label("Maintainer");
    let recent_detail_maintainer_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .ellipsize(pango::EllipsizeMode::None)
        .visible(false)
        .build();
    recent_detail_maintainer_value.set_hexpand(true);
    recent_detail_maintainer_value.set_xalign(0.0);
    recent_detail_maintainer_value.set_selectable(true);
    recent_detail_maintainer_row.append(&recent_detail_maintainer_title);
    recent_detail_maintainer_row.append(&recent_detail_maintainer_value);
    recent_detail_metadata_box.append(&recent_detail_maintainer_row);

    let recent_detail_license_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let recent_detail_license_title = make_recent_metadata_label("License");
    let recent_detail_license_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .ellipsize(pango::EllipsizeMode::None)
        .visible(false)
        .build();
    recent_detail_license_value.set_hexpand(true);
    recent_detail_license_value.set_xalign(0.0);
    recent_detail_license_value.set_selectable(true);
    recent_detail_license_row.append(&recent_detail_license_title);
    recent_detail_license_row.append(&recent_detail_license_value);
    recent_detail_metadata_box.append(&recent_detail_license_row);

    let recent_detail_updated_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let recent_detail_updated_title = make_recent_metadata_label("Updated");
    let recent_detail_updated_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .visible(false)
        .build();
    recent_detail_updated_value.set_hexpand(true);
    recent_detail_updated_value.set_xalign(0.0);
    recent_detail_updated_row.append(&recent_detail_updated_title);
    recent_detail_updated_row.append(&recent_detail_updated_value);
    recent_detail_metadata_box.append(&recent_detail_updated_row);

    let recent_detail_status = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .visible(false)
        .build();
    recent_detail_status.add_css_class("dim-label");

    let recent_detail_update_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .visible(false)
        .build();
    recent_detail_update_label.add_css_class("accent");

    let recent_detail_description = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::Word)
        .hexpand(true)
        .justify(gtk::Justification::Fill)
        .build();
    recent_detail_description.set_text("Select a package to see details.");
    recent_detail_description.set_ellipsize(pango::EllipsizeMode::None);
    recent_detail_description.set_single_line_mode(false);

    let recent_detail_description_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    let recent_detail_description_title = make_recent_metadata_label("Description");
    let recent_detail_description_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .halign(gtk::Align::Fill)
        .build();
    recent_detail_description_container.append(&recent_detail_description);
    recent_detail_description_row.append(&recent_detail_description_title);
    recent_detail_description_row.append(&recent_detail_description_container);

    let recent_detail_actions_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Start)
        .build();
    recent_detail_actions_row.set_margin_top(6);
    recent_detail_actions_row.append(&recent_detail_action_button);

    let recent_detail_dependencies_list = gtk::ListBox::new();
    recent_detail_dependencies_list.add_css_class("boxed-list");
    recent_detail_dependencies_list.set_selection_mode(gtk::SelectionMode::None);
    recent_detail_dependencies_list.set_activate_on_single_click(true);
    recent_detail_dependencies_list.set_visible(false);

    let recent_detail_dependencies_placeholder = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    recent_detail_dependencies_placeholder.add_css_class("dim-label");
    recent_detail_dependencies_placeholder.set_text("No runtime dependencies.");

    let recent_detail_dependencies_stack = gtk::Stack::new();
    recent_detail_dependencies_stack
        .add_named(&recent_detail_dependencies_placeholder, Some("placeholder"));
    recent_detail_dependencies_stack.add_named(&recent_detail_dependencies_list, Some("list"));
    recent_detail_dependencies_stack.set_visible_child_name("placeholder");

    let recent_detail_dependencies_group = adw::PreferencesGroup::builder()
        .title("Dependencies")
        .build();
    recent_detail_dependencies_group.add(&recent_detail_dependencies_stack);

    let recent_detail_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .hexpand(true)
        .margin_top(0)
        .margin_bottom(0)
        .margin_start(0)
        .margin_end(0)
        .build();
    recent_detail_box.add_css_class("background");
    recent_detail_box.add_css_class("nebula-card");
    recent_detail_box.append(&recent_detail_header);
    recent_detail_box.append(&recent_detail_status);
    recent_detail_box.append(&recent_detail_metadata_box);
    recent_detail_box.append(&recent_detail_update_label);
    recent_detail_box.append(&recent_detail_description_row);
    recent_detail_box.append(&recent_detail_actions_row);
    recent_detail_box.append(&recent_detail_dependencies_group);

    let recent_detail_scroller = gtk::ScrolledWindow::builder()
        .hexpand(false)
        .vexpand(true)
        .min_content_height(0)
        .propagate_natural_height(true)
        .build();
    recent_detail_scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    recent_detail_scroller.set_propagate_natural_height(true);
    recent_detail_scroller.set_child(Some(&recent_detail_box));
    recent_detail_scroller.set_halign(gtk::Align::Fill);
    recent_detail_scroller.set_valign(gtk::Align::Fill);
    recent_detail_scroller.set_hexpand(true);
    recent_detail_scroller.set_vexpand(true);
    recent_detail_scroller.set_margin_start(0);
    recent_detail_scroller.set_margin_end(0);
    recent_detail_scroller.set_margin_top(0);
    recent_detail_scroller.set_margin_bottom(0);

    let recent_detail_revealer = gtk::Revealer::builder()
        .reveal_child(false)
        .transition_type(gtk::RevealerTransitionType::SlideLeft)
        .build();
    recent_detail_revealer.set_halign(gtk::Align::Fill);
    recent_detail_revealer.set_valign(gtk::Align::Fill);
    recent_detail_revealer.set_hexpand(true);
    recent_detail_revealer.set_vexpand(true);
    recent_detail_revealer.set_child(Some(&recent_detail_scroller));
    spotlight_recent_overlay.add_overlay(&recent_detail_revealer);
    spotlight_recent_stack.set_visible_child_name("placeholder");

    let recent_heading = gtk::Label::builder()
        .label("Recent repository updates")
        .halign(gtk::Align::Start)
        .build();
    recent_heading.add_css_class("title-2");
    recent_heading.set_margin_bottom(0);
    recent_heading.set_valign(gtk::Align::Center);

    let recent_refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh recently updated")
        .build();
    recent_refresh_button.add_css_class("flat");
    recent_refresh_button.set_focus_on_click(false);
    recent_refresh_button.set_valign(gtk::Align::Center);

    let recent_header_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Start)
        .build();
    recent_header_row.set_valign(gtk::Align::Center);
    recent_header_row.append(&recent_heading);
    recent_header_row.append(&recent_refresh_button);

    let recent_group = adw::PreferencesGroup::new();
    recent_group.set_title("");
    recent_group.set_valign(gtk::Align::Fill);
    recent_group.set_vexpand(true);
    recent_group.add(&spotlight_recent_stack);

    let categories_heading = gtk::Label::builder()
        .label("Categories")
        .halign(gtk::Align::Start)
        .build();
    categories_heading.add_css_class("title-2");
    categories_heading.set_margin_bottom(4);

    let categories_group = adw::PreferencesGroup::new();
    categories_group.set_title("");
    categories_group.set_hexpand(false);
    categories_group.set_valign(gtk::Align::Start);
    categories_group.add(&categories_list);

    let categories_column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .halign(gtk::Align::Start)
        .build();
    categories_column.set_hexpand(false);
    categories_column.set_valign(gtk::Align::Start);
    categories_column.append(&categories_heading);
    categories_column.append(&categories_group);

    let recent_column = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .hexpand(true)
        .halign(gtk::Align::Fill)
        .build();
    recent_column.set_valign(gtk::Align::Fill);
    recent_column.append(&recent_header_row);
    recent_column.append(&recent_group);

    let spotlight_columns = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(24)
        .hexpand(true)
        .build();
    spotlight_columns.append(&categories_column);
    spotlight_columns.append(&recent_column);

    spotlight_section_box.append(&spotlight_columns);

    let status_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .hexpand(true)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    status_label.set_margin_top(6);
    status_label.set_margin_bottom(6);
    status_label.set_visible(false);

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(false);

    let scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(320)
        .build();
    scroller.set_child(Some(&list));
    scroller.set_visible(false);
    scroller.set_vexpand(true);

    let detail_name = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .hexpand(true)
        .build();
    detail_name.add_css_class("title-2");
    detail_name.set_xalign(0.0);

    let detail_back_button = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text("Go back to the previous package")
        .has_frame(false)
        .visible(false)
        .sensitive(false)
        .build();
    detail_back_button.add_css_class("flat");
    detail_back_button.set_focus_on_click(false);
    detail_back_button.set_valign(gtk::Align::Center);

    let detail_action_button = gtk::Button::builder()
        .label("Install")
        .width_request(140)
        .build();
    detail_action_button.add_css_class("suggested-action");
    detail_action_button.set_visible(false);
    detail_action_button.set_halign(gtk::Align::Start);

    let detail_close_button = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Close details")
        .has_frame(false)
        .visible(false)
        .sensitive(false)
        .build();
    detail_close_button.add_css_class("flat");
    detail_close_button.set_focus_on_click(false);
    detail_close_button.set_valign(gtk::Align::Center);

    let detail_header_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    let detail_header_spacer = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .build();
    detail_header_row.append(&detail_back_button);
    detail_header_row.append(&detail_name);
    detail_header_row.append(&detail_header_spacer);
    detail_header_row.append(&detail_close_button);

    let detail_metadata_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();

    let make_metadata_label = |text: &str| {
        let label = gtk::Label::builder()
            .label(text)
            .halign(gtk::Align::Start)
            .build();
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        label.set_width_chars(14);
        label
    };

    let detail_version_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    let detail_version_title = make_metadata_label("Version");
    let detail_version_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    detail_version_value.set_hexpand(true);
    detail_version_value.set_xalign(0.0);
    detail_version_row.append(&detail_version_title);
    detail_version_row.append(&detail_version_value);
    detail_metadata_box.append(&detail_version_row);

    let detail_download_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    let detail_download_title = make_metadata_label("Download size");
    let detail_download_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    detail_download_value.set_text("—");
    detail_download_value.set_hexpand(true);
    detail_download_value.set_xalign(0.0);
    detail_download_row.append(&detail_download_title);
    detail_download_row.append(&detail_download_value);
    detail_metadata_box.append(&detail_download_row);

    let detail_repository_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_repository_title = make_metadata_label("Repository");
    let detail_repository_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .visible(false)
        .build();
    detail_repository_value.set_hexpand(true);
    detail_repository_value.set_xalign(0.0);
    detail_repository_row.append(&detail_repository_title);
    detail_repository_row.append(&detail_repository_value);
    detail_metadata_box.append(&detail_repository_row);

    let detail_homepage_link = gtk::LinkButton::builder()
        .label("")
        .halign(gtk::Align::Start)
        .has_frame(false)
        .visible(false)
        .build();
    detail_homepage_link.set_hexpand(true);
    let detail_homepage_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_homepage_title = make_metadata_label("Homepage");
    detail_homepage_row.append(&detail_homepage_title);
    detail_homepage_row.append(&detail_homepage_link);
    detail_metadata_box.append(&detail_homepage_row);

    let detail_maintainer_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_maintainer_title = make_metadata_label("Maintainer");
    let detail_maintainer_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .ellipsize(pango::EllipsizeMode::None)
        .visible(false)
        .build();
    detail_maintainer_value.set_hexpand(true);
    detail_maintainer_value.set_xalign(0.0);
    detail_maintainer_value.set_selectable(true);
    detail_maintainer_row.append(&detail_maintainer_title);
    detail_maintainer_row.append(&detail_maintainer_value);
    detail_metadata_box.append(&detail_maintainer_row);

    let detail_license_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_license_title = make_metadata_label("License");
    let detail_license_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .ellipsize(pango::EllipsizeMode::None)
        .visible(false)
        .build();
    detail_license_value.set_hexpand(true);
    detail_license_value.set_xalign(0.0);
    detail_license_value.set_selectable(true);
    detail_license_row.append(&detail_license_title);
    detail_license_row.append(&detail_license_value);
    detail_metadata_box.append(&detail_license_row);

    let detail_update_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_update_label.add_css_class("accent");
    detail_update_label.set_visible(false);

    let detail_update_button = gtk::Button::builder()
        .label("Update")
        .width_request(120)
        .build();
    detail_update_button.add_css_class("suggested-action");
    detail_update_button.set_visible(false);
    detail_update_button.set_halign(gtk::Align::Start);
    detail_update_button.set_valign(gtk::Align::Center);
    detail_update_button.set_margin_start(0);
    detail_update_button.set_tooltip_text(Some("Install this update."));

    let detail_description = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::Word)
        .hexpand(true)
        .justify(gtk::Justification::Fill)
        .build();
    detail_description.set_text("Select a package to see details.");
    detail_description.set_ellipsize(pango::EllipsizeMode::None);
    detail_description.set_single_line_mode(false);

    let detail_description_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    let detail_description_title = make_metadata_label("Description");
    let detail_description_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .halign(gtk::Align::Fill)
        .build();
    detail_description_container.append(&detail_description);
    detail_description_row.append(&detail_description_title);
    detail_description_row.append(&detail_description_container);

    let detail_actions_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Start)
        .build();
    detail_actions_row.set_margin_top(6);
    detail_actions_row.append(&detail_action_button);
    detail_actions_row.append(&detail_update_button);

    let detail_dependencies_list = gtk::ListBox::new();
    detail_dependencies_list.add_css_class("boxed-list");
    detail_dependencies_list.set_selection_mode(gtk::SelectionMode::None);
    detail_dependencies_list.set_activate_on_single_click(true);
    detail_dependencies_list.set_visible(false);

    let detail_dependencies_placeholder = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_dependencies_placeholder.add_css_class("dim-label");
    detail_dependencies_placeholder.set_text("No runtime dependencies.");

    let detail_dependencies_stack = gtk::Stack::new();
    detail_dependencies_stack.add_named(&detail_dependencies_placeholder, Some("placeholder"));
    detail_dependencies_stack.add_named(&detail_dependencies_list, Some("list"));
    detail_dependencies_stack.set_visible_child_name("placeholder");

    let detail_dependencies_group = adw::PreferencesGroup::builder()
        .title("Dependencies")
        .build();
    detail_dependencies_group.add(&detail_dependencies_stack);

    let detail_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .hexpand(true)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(16)
        .build();
    detail_box.add_css_class("nebula-card");
    detail_box.add_css_class("compact");
    detail_box.append(&detail_header_row);
    detail_box.append(&detail_metadata_box);
    detail_box.append(&detail_update_label);
    detail_box.append(&detail_description_row);
    detail_box.append(&detail_actions_row);
    detail_box.append(&detail_dependencies_group);

    let detail_scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(320)
        .build();
    detail_scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    detail_scroller.set_child(Some(&detail_box));

    let detail_placeholder = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_placeholder.add_css_class("dim-label");
    detail_placeholder.set_text("Select a package to see details.");
    detail_placeholder.set_hexpand(true);
    detail_placeholder.set_vexpand(true);

    let detail_stack = gtk::Stack::new();
    detail_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    detail_stack.set_hexpand(true);
    detail_stack.set_vexpand(true);
    detail_stack.add_named(&detail_placeholder, Some("placeholder"));
    detail_stack.add_named(&detail_scroller, Some("detail"));
    detail_stack.set_visible_child_name("placeholder");

    let detail_frame = gtk::Frame::builder().hexpand(true).vexpand(true).build();
    detail_frame.set_child(Some(&detail_stack));
    detail_frame.set_visible(false);

    let content_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .hexpand(true)
        .vexpand(true)
        .build();
    content_row.set_homogeneous(false);
    content_row.append(&scroller);
    content_row.append(&detail_frame);
    content_row.set_visible(false);

    container.append(&search_row);
    container.append(&spotlight_section_box);
    container.append(&status_label);
    container.append(&content_row);
    container.append(&spotlight_status_row);

    let widgets = DiscoverWidgets {
        search_entry,
        search_button,
        search_spinner,
        status_label,
        list,
        scroller,
        content_row,
        detail_stack,
        detail_name,
        detail_back_button,
        detail_close_button,
        detail_version_value,
        detail_repository_row,
        detail_repository_value,
        detail_description,
        detail_download_value,
        detail_homepage_row,
        detail_homepage_link,
        detail_maintainer_row,
        detail_maintainer_value,
        detail_license_row,
        detail_license_value,
        detail_update_label,
        detail_action_button,
        detail_dependencies_stack,
        detail_dependencies_list,
        detail_dependencies_placeholder,
        detail_frame,
        spotlight_spinner,
        spotlight_status,
        spotlight_recent_stack,
        spotlight_recent_list,
        spotlight_recent_scroller: spotlight_recent_scroller.clone(),
        spotlight_recent_detail_revealer: recent_detail_revealer.clone(),
        spotlight_recent_detail_container: recent_detail_box.clone(),
        spotlight_recent_back_button: recent_detail_back_button.clone(),
        spotlight_recent_detail_name: recent_detail_name.clone(),
        spotlight_recent_detail_spinner: recent_detail_spinner.clone(),
        spotlight_recent_detail_version_value: recent_detail_version_value.clone(),
        spotlight_recent_detail_repo_row: recent_detail_repo_row.clone(),
        spotlight_recent_detail_repo_value: recent_detail_repo_value.clone(),
        spotlight_recent_detail_download_value: recent_detail_download_value.clone(),
        spotlight_recent_detail_updated_row: recent_detail_updated_row.clone(),
        spotlight_recent_detail_updated_value: recent_detail_updated_value.clone(),
        spotlight_recent_detail_homepage_row: recent_detail_homepage_row.clone(),
        spotlight_recent_detail_homepage_link: recent_detail_homepage_link.clone(),
        spotlight_recent_detail_maintainer_row: recent_detail_maintainer_row.clone(),
        spotlight_recent_detail_maintainer_value: recent_detail_maintainer_value.clone(),
        spotlight_recent_detail_license_row: recent_detail_license_row.clone(),
        spotlight_recent_detail_license_value: recent_detail_license_value.clone(),
        spotlight_recent_detail_status: recent_detail_status.clone(),
        spotlight_recent_detail_description: recent_detail_description.clone(),
        spotlight_recent_detail_update_label: recent_detail_update_label.clone(),
        spotlight_recent_detail_dependencies_stack: recent_detail_dependencies_stack.clone(),
        spotlight_recent_detail_dependencies_list: recent_detail_dependencies_list.clone(),
        spotlight_recent_detail_dependencies_placeholder: recent_detail_dependencies_placeholder
            .clone(),
        spotlight_recent_action_button: recent_detail_action_button.clone(),
        spotlight_section_box,
        category_browsers_button,
        category_chat_button,
        category_email_button,
        category_games_button,
        category_graphics_button,
        category_music_button,
        category_productivity_button,
        category_utilities_button,
        category_video_button,
        spotlight_refresh_button: recent_refresh_button,
    };

    (container, widgets)
}

fn build_installed_page() -> (gtk::Box, InstalledWidgets) {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    container.set_vexpand(true);

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search installed packages")
        .hexpand(true)
        .build();

    let filter_model = gtk::StringList::new(&["All packages", "Updates available"]);
    let filter_dropdown = gtk::DropDown::builder()
        .model(&filter_model)
        .selected(0)
        .build();
    filter_dropdown.set_hexpand(false);
    filter_dropdown.add_css_class("nebula-compact-dropdown");

    let controls_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .hexpand(true)
        .build();
    controls_row.append(&search_entry);
    controls_row.append(&filter_dropdown);

    let status_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(false)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    status_label.set_text("");
    status_label.set_hexpand(true);
    status_label.set_max_width_chars(200);
    status_label.set_xalign(0.0);

    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);
    spinner.set_valign(gtk::Align::Center);

    let refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh installed packages")
        .build();
    refresh_button.add_css_class("flat");
    refresh_button.set_focus_on_click(false);

    let remove_selected_button = gtk::Button::builder()
        .label("Remove Selected")
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    remove_selected_button.add_css_class("destructive-action");

    let status_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .hexpand(true)
        .build();
    status_row.set_halign(gtk::Align::Fill);
    status_row.set_baseline_position(gtk::BaselinePosition::Center);
    status_row.append(&refresh_button);
    status_row.append(&status_label);
    status_row.append(&spinner);
    status_row.append(&remove_selected_button);

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(false);
    list.set_focusable(false);

    let scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(320)
        .build();
    scroller.set_child(Some(&list));

    let detail_name = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_name.add_css_class("title-2");
    detail_name.set_hexpand(true);
    detail_name.set_xalign(0.0);
    detail_name.set_hexpand(true);
    detail_name.set_xalign(0.0);
    detail_name.set_hexpand(true);
    detail_name.set_xalign(0.0);
    detail_name.set_hexpand(true);
    detail_name.set_xalign(0.0);
    detail_name.set_text("");

    let detail_back_button = gtk::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text("Go back to the previous package")
        .has_frame(false)
        .visible(false)
        .sensitive(false)
        .build();
    detail_back_button.add_css_class("flat");
    detail_back_button.set_focus_on_click(false);
    detail_back_button.set_valign(gtk::Align::Center);

    let detail_close_button = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Close details")
        .has_frame(false)
        .visible(false)
        .sensitive(false)
        .build();
    detail_close_button.add_css_class("flat");
    detail_close_button.set_focus_on_click(false);
    detail_close_button.set_valign(gtk::Align::Center);

    let detail_header_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    let detail_header_row_spacer = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .build();
    detail_header_row.append(&detail_back_button);
    detail_header_row.append(&detail_name);
    detail_header_row.append(&detail_header_row_spacer);
    detail_header_row.append(&detail_close_button);

    let detail_metadata_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();

    let make_metadata_label = |text: &str| {
        let label = gtk::Label::builder()
            .label(text)
            .halign(gtk::Align::Start)
            .build();
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        label.set_width_chars(14);
        label
    };

    let detail_version_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    let detail_version_title = make_metadata_label("Version");
    let detail_version_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    detail_version_value.set_hexpand(true);
    detail_version_value.set_xalign(0.0);
    detail_version_row.append(&detail_version_title);
    detail_version_row.append(&detail_version_value);
    detail_metadata_box.append(&detail_version_row);

    let detail_download_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    let detail_download_title = make_metadata_label("Install size");
    let detail_download_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    detail_download_value.set_text("—");
    detail_download_value.set_hexpand(true);
    detail_download_value.set_xalign(0.0);
    detail_download_row.append(&detail_download_title);
    detail_download_row.append(&detail_download_value);
    detail_metadata_box.append(&detail_download_row);

    let detail_homepage_link = gtk::LinkButton::builder()
        .label("")
        .halign(gtk::Align::Start)
        .has_frame(false)
        .visible(false)
        .build();
    detail_homepage_link.set_hexpand(true);
    let detail_homepage_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_homepage_title = make_metadata_label("Homepage");
    detail_homepage_row.append(&detail_homepage_title);
    detail_homepage_row.append(&detail_homepage_link);
    detail_metadata_box.append(&detail_homepage_row);

    let detail_maintainer_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .visible(false)
        .build();
    detail_maintainer_value.set_hexpand(true);
    detail_maintainer_value.set_xalign(0.0);
    let detail_maintainer_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_maintainer_title = make_metadata_label("Maintainer");
    detail_maintainer_row.append(&detail_maintainer_title);
    detail_maintainer_row.append(&detail_maintainer_value);
    detail_metadata_box.append(&detail_maintainer_row);

    let detail_license_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .ellipsize(pango::EllipsizeMode::None)
        .visible(false)
        .build();
    detail_license_value.set_hexpand(true);
    detail_license_value.set_xalign(0.0);
    detail_license_value.set_single_line_mode(false);
    let detail_license_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_license_title = make_metadata_label("License");
    detail_license_row.append(&detail_license_title);
    detail_license_row.append(&detail_license_value);
    detail_metadata_box.append(&detail_license_row);

    let detail_update_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_update_label.add_css_class("accent");

    let detail_description = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::Word)
        .hexpand(true)
        .justify(gtk::Justification::Fill)
        .build();
    detail_description.set_text("");
    detail_description.set_ellipsize(pango::EllipsizeMode::None);
    detail_description.set_single_line_mode(false);

    let detail_description_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    let detail_description_title = make_metadata_label("Description");
    detail_description_title.set_width_chars(14);
    let description_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .halign(gtk::Align::Fill)
        .build();
    description_container.append(&detail_description);
    detail_description_row.append(&detail_description_title);
    detail_description_row.append(&description_container);

    let detail_remove_button = gtk::Button::builder()
        .label("Remove")
        .width_request(120)
        .build();
    detail_remove_button.add_css_class("destructive-action");
    detail_remove_button.set_halign(gtk::Align::Start);
    detail_remove_button.set_visible(false);
    detail_remove_button.set_valign(gtk::Align::Center);
    detail_remove_button.set_tooltip_text(Some("Remove this package."));

    let detail_update_button = gtk::Button::builder()
        .label("Update")
        .width_request(120)
        .build();
    detail_update_button.add_css_class("suggested-action");
    detail_update_button.set_halign(gtk::Align::Start);
    detail_update_button.set_visible(false);
    detail_update_button.set_valign(gtk::Align::Center);
    detail_update_button.set_margin_start(0);
    detail_update_button.set_tooltip_text(Some("Install the available update."));

    let detail_header_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    detail_header_container.append(&detail_header_row);

    let detail_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .hexpand(true)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(16)
        .build();
    detail_box.add_css_class("nebula-card");
    detail_box.add_css_class("compact");
    detail_box.append(&detail_header_container);
    detail_box.append(&detail_metadata_box);
    detail_box.append(&detail_update_label);
    detail_box.append(&detail_description_row);

    let detail_actions_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Start)
        .build();
    detail_actions_row.set_margin_top(6);
    detail_actions_row.append(&detail_remove_button);
    detail_actions_row.append(&detail_update_button);
    detail_box.append(&detail_actions_row);

    let detail_required_by_placeholder = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_required_by_placeholder.add_css_class("dim-label");
    detail_required_by_placeholder.set_text("Not required by any installed package.");

    let detail_required_by_list = gtk::ListBox::new();
    detail_required_by_list.add_css_class("boxed-list");
    detail_required_by_list.set_selection_mode(gtk::SelectionMode::None);
    detail_required_by_list.set_activate_on_single_click(true);
    detail_required_by_list.set_visible(false);

    let detail_required_by_stack = gtk::Stack::new();
    detail_required_by_stack.add_named(&detail_required_by_placeholder, Some("placeholder"));
    detail_required_by_stack.add_named(&detail_required_by_list, Some("list"));
    detail_required_by_stack.set_visible_child_name("placeholder");

    let detail_required_by_group = adw::PreferencesGroup::builder()
        .title("Required By")
        .build();
    detail_required_by_group.add(&detail_required_by_stack);
    detail_box.append(&detail_required_by_group);

    let detail_scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(320)
        .build();
    detail_scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    detail_scroller.set_child(Some(&detail_box));

    let detail_placeholder = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_placeholder.add_css_class("dim-label");
    detail_placeholder.set_text("Select a package to see details.");
    detail_placeholder.set_hexpand(true);
    detail_placeholder.set_vexpand(true);
    detail_placeholder.set_justify(gtk::Justification::Center);

    let detail_stack = gtk::Stack::new();
    detail_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    detail_stack.set_hexpand(true);
    detail_stack.set_vexpand(true);
    detail_stack.add_named(&detail_placeholder, Some("placeholder"));
    detail_stack.add_named(&detail_scroller, Some("detail"));
    detail_stack.set_visible_child_name("placeholder");

    let detail_frame = gtk::Frame::builder().hexpand(true).vexpand(true).build();
    detail_frame.set_child(Some(&detail_stack));
    detail_frame.set_visible(false);

    let content_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .hexpand(true)
        .vexpand(true)
        .build();
    content_row.set_homogeneous(false);
    content_row.append(&scroller);
    content_row.append(&detail_frame);

    container.append(&controls_row);
    container.append(&status_row);
    container.append(&content_row);
    let footer_label = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    footer_label.add_css_class("dim-label");
    footer_label.set_text("Last refreshed —");
    footer_label.set_hexpand(true);
    footer_label.set_xalign(0.5);
    container.append(&footer_label);

    let widgets = InstalledWidgets {
        refresh_button,
        search_entry,
        status_label,
        spinner,
        filter_dropdown,
        remove_selected_button,
        list,
        detail_stack,
        detail_frame,
        detail_remove_button,
        detail_update_button,
        detail_back_button,
        detail_close_button,
        detail_name,
        detail_version_value,
        detail_description,
        detail_download_value,
        detail_homepage_row,
        detail_homepage_link,
        detail_maintainer_row,
        detail_maintainer_value,
        detail_license_row,
        detail_license_value,
        detail_required_by_stack,
        detail_required_by_list,
        detail_required_by_placeholder,
        detail_update_label,
        footer_label,
    };

    (container, widgets)
}

fn build_updates_page() -> (gtk::Box, UpdatesWidgets) {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    container.set_vexpand(true);

    let summary_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .hexpand(true)
        .build();

    let summary_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    summary_label.add_css_class("dim-label");
    summary_label.set_hexpand(true);
    summary_label.set_valign(gtk::Align::Center);
    summary_label.set_text("No updates checked yet.");
    summary_row.append(&summary_label);

    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);
    spinner.set_hexpand(false);
    spinner.set_valign(gtk::Align::Center);
    summary_row.append(&spinner);

    let status_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    status_label.set_text("Check for updates to see what’s new.");

    let placeholder = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .build();
    placeholder.set_valign(gtk::Align::Center);
    placeholder.set_halign(gtk::Align::Center);
    placeholder.set_vexpand(true);

    let placeholder_label = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    placeholder_label.add_css_class("title-4");
    placeholder_label.set_text("Your system is up to date!");
    placeholder.append(&placeholder_label);

    let check_button = gtk::Button::builder().label("Check for updates").build();
    check_button.add_css_class("suggested-action");
    placeholder.append(&check_button);

    let refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Center)
        .tooltip_text("Check for new updates")
        .build();
    refresh_button.set_focus_on_click(false);
    refresh_button.add_css_class("flat");

    let update_all_button = gtk::Button::builder()
        .label("Update All")
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    update_all_button.add_css_class("suggested-action");
    update_all_button.set_visible(false);
    update_all_button.set_margin_start(12);

    let controls_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .hexpand(true)
        .build();
    controls_row.set_valign(gtk::Align::Center);
    controls_row.set_halign(gtk::Align::Fill);
    controls_row.append(&refresh_button);
    controls_row.append(&summary_row);
    controls_row.append(&update_all_button);

    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.set_activate_on_single_click(false);
    list.set_focusable(false);

    let scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(320)
        .build();
    scroller.set_child(Some(&list));
    scroller.set_visible(false);

    let detail_name = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_name.add_css_class("title-2");

    let detail_close_button = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .tooltip_text("Close details")
        .has_frame(false)
        .visible(false)
        .sensitive(false)
        .build();
    detail_close_button.add_css_class("flat");
    detail_close_button.set_focus_on_click(false);
    detail_close_button.set_valign(gtk::Align::Center);
    detail_close_button.set_halign(gtk::Align::End);

    let detail_header_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    detail_header_row.set_hexpand(true);
    let detail_header_row_spacer = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .hexpand(true)
        .build();
    detail_header_row.append(&detail_name);
    detail_header_row.append(&detail_header_row_spacer);
    detail_header_row.append(&detail_close_button);

    let make_metadata_label = |text: &str| {
        let label = gtk::Label::builder()
            .label(text)
            .halign(gtk::Align::Start)
            .build();
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        label.set_width_chars(14);
        label
    };

    let detail_metadata_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();

    let detail_version_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    let detail_version_title = make_metadata_label("Version");
    let detail_version_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    detail_version_value.set_hexpand(true);
    detail_version_value.set_xalign(0.0);
    detail_version_row.append(&detail_version_title);
    detail_version_row.append(&detail_version_value);
    detail_metadata_box.append(&detail_version_row);

    let detail_download_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    let detail_download_title = make_metadata_label("Install size");
    let detail_download_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .single_line_mode(true)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    detail_download_value.set_text("—");
    detail_download_value.set_hexpand(true);
    detail_download_value.set_xalign(0.0);
    detail_download_row.append(&detail_download_title);
    detail_download_row.append(&detail_download_value);
    detail_metadata_box.append(&detail_download_row);

    let detail_homepage_link = gtk::LinkButton::builder()
        .label("")
        .halign(gtk::Align::Start)
        .has_frame(false)
        .visible(false)
        .build();
    detail_homepage_link.set_hexpand(true);
    let detail_homepage_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_homepage_title = make_metadata_label("Homepage");
    detail_homepage_row.append(&detail_homepage_title);
    detail_homepage_row.append(&detail_homepage_link);
    detail_metadata_box.append(&detail_homepage_row);

    let detail_maintainer_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .ellipsize(pango::EllipsizeMode::None)
        .visible(false)
        .build();
    detail_maintainer_value.set_hexpand(true);
    detail_maintainer_value.set_xalign(0.0);
    detail_maintainer_value.set_selectable(true);
    let detail_maintainer_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_maintainer_title = make_metadata_label("Maintainer");
    detail_maintainer_row.append(&detail_maintainer_title);
    detail_maintainer_row.append(&detail_maintainer_value);
    detail_metadata_box.append(&detail_maintainer_row);

    let detail_license_value = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .ellipsize(pango::EllipsizeMode::None)
        .visible(false)
        .build();
    detail_license_value.set_hexpand(true);
    detail_license_value.set_xalign(0.0);
    detail_license_value.set_selectable(true);
    let detail_license_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .visible(false)
        .build();
    let detail_license_title = make_metadata_label("License");
    detail_license_row.append(&detail_license_title);
    detail_license_row.append(&detail_license_value);
    detail_metadata_box.append(&detail_license_row);

    let detail_update_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_update_label.add_css_class("accent");
    detail_update_label.set_visible(false);

    let detail_update_button = gtk::Button::builder()
        .label("Update")
        .width_request(120)
        .build();
    detail_update_button.add_css_class("suggested-action");
    detail_update_button.set_visible(false);
    detail_update_button.set_halign(gtk::Align::Start);
    detail_update_button.set_valign(gtk::Align::Center);
    detail_update_button.set_margin_start(0);
    detail_update_button.set_tooltip_text(Some("Install this update."));

    let detail_description = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::Word)
        .hexpand(true)
        .justify(gtk::Justification::Fill)
        .build();
    detail_description.set_text("Select an update to see details.");
    detail_description.set_ellipsize(pango::EllipsizeMode::None);
    detail_description.set_single_line_mode(false);

    let detail_description_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .hexpand(true)
        .build();
    let detail_description_title = make_metadata_label("Description");
    let detail_description_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .halign(gtk::Align::Fill)
        .build();
    detail_description_container.append(&detail_description);
    detail_description_row.append(&detail_description_title);
    detail_description_row.append(&detail_description_container);

    let detail_header_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::Fill)
        .build();
    detail_header_container.set_hexpand(true);
    detail_header_container.append(&detail_header_row);

    let detail_actions_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::Start)
        .build();
    detail_actions_row.set_margin_top(6);
    detail_actions_row.append(&detail_update_button);

    let detail_required_by_placeholder = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_required_by_placeholder.add_css_class("dim-label");
    detail_required_by_placeholder.set_text("Not required by any installed package.");

    let detail_required_by_list = gtk::ListBox::new();
    detail_required_by_list.add_css_class("boxed-list");
    detail_required_by_list.set_selection_mode(gtk::SelectionMode::None);
    detail_required_by_list.set_activate_on_single_click(true);
    detail_required_by_list.set_visible(false);

    let detail_required_by_stack = gtk::Stack::new();
    detail_required_by_stack.add_named(&detail_required_by_placeholder, Some("placeholder"));
    detail_required_by_stack.add_named(&detail_required_by_list, Some("list"));
    detail_required_by_stack.set_visible_child_name("placeholder");

    let detail_required_by_group = adw::PreferencesGroup::builder()
        .title("Required By")
        .build();
    detail_required_by_group.add(&detail_required_by_stack);

    let detail_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .hexpand(true)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(10)
        .margin_end(16)
        .build();
    detail_box.append(&detail_header_container);
    detail_box.append(&detail_metadata_box);
    detail_box.append(&detail_update_label);
    detail_box.append(&detail_description_row);
    detail_box.append(&detail_actions_row);
    detail_box.append(&detail_required_by_group);

    let detail_scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(320)
        .build();
    detail_scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    detail_scroller.set_child(Some(&detail_box));

    let detail_placeholder = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_placeholder.add_css_class("dim-label");
    detail_placeholder.set_text("Select an update to see details.");
    detail_placeholder.set_hexpand(true);
    detail_placeholder.set_vexpand(true);

    let detail_stack = gtk::Stack::new();
    detail_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    detail_stack.set_hexpand(true);
    detail_stack.set_vexpand(true);
    detail_stack.add_named(&detail_placeholder, Some("placeholder"));
    detail_stack.add_named(&detail_scroller, Some("detail"));
    detail_stack.set_visible_child_name("placeholder");

    let detail_frame = gtk::Frame::builder().hexpand(true).vexpand(true).build();
    detail_frame.set_child(Some(&detail_stack));
    detail_frame.set_visible(false);

    let content_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .hexpand(true)
        .vexpand(true)
        .build();
    content_row.set_homogeneous(false);
    content_row.append(&scroller);
    content_row.append(&detail_frame);
    content_row.set_visible(false);

    container.append(&controls_row);
    container.append(&status_label);
    container.append(&placeholder);
    let footer_label = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    footer_label.add_css_class("dim-label");
    footer_label.set_text("Last checked — never.");

    container.append(&content_row);
    container.append(&footer_label);

    let widgets = UpdatesWidgets {
        summary_row: summary_row.clone(),
        status_label,
        list,
        scroller,
        content_row,
        placeholder,
        placeholder_label,
        check_button,
        refresh_button,
        update_all_button,
        spinner,
        summary_label,
        footer_label,
        detail_frame,
        detail_stack,
        detail_name,
        detail_close_button,
        detail_version_value,
        detail_download_value,
        detail_homepage_row,
        detail_homepage_link,
        detail_maintainer_row,
        detail_maintainer_value,
        detail_license_row,
        detail_license_value,
        detail_description,
        detail_update_label,
        detail_required_by_stack,
        detail_required_by_list,
        detail_required_by_placeholder,
        detail_update_button,
    };

    (container, widgets)
}

fn build_package_row(pkg: &PackageInfo) -> adw::ActionRow {
    let subtitle = if pkg.description.is_empty() {
        pkg.version.clone()
    } else if pkg.version.is_empty() {
        pkg.description.clone()
    } else {
        format!("{} - {}", pkg.version, pkg.description)
    };

    let row = adw::ActionRow::builder()
        .title(pkg.name.as_str())
        .subtitle(subtitle.as_str())
        .build();
    row.set_title_lines(1);
    row.set_subtitle_lines(2);
    row.set_activatable(true);
    row.set_focusable(true);
    row.set_tooltip_text(Some("Open details for this package."));

    let suffix_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();

    if pkg.installed {
        let badge = gtk::Label::new(Some("Installed"));
        badge.add_css_class("tag");
        suffix_box.append(&badge);
    }

    if let Some(build_date) = pkg.build_date {
        let relative = format_relative_time(build_date);
        let time_label = gtk::Label::builder()
            .label(relative.as_str())
            .halign(gtk::Align::End)
            .build();
        time_label.add_css_class("dim-label");
        time_label.set_justify(gtk::Justification::Right);
        time_label.set_xalign(1.0);
        suffix_box.append(&time_label);
    }

    if suffix_box.first_child().is_some() {
        row.add_suffix(&suffix_box);
    }

    row
}

fn run_xbps_query_dependencies(package: &str) -> Result<Vec<DependencyInfo>, String> {
    let output = Command::new("xbps-query")
        .args(["-R", "--show", package])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut dependencies = Vec::new();
    let mut in_run_depends = false;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(spec) = trimmed.strip_prefix("run_depends:") {
            in_run_depends = true;
            let spec = spec.trim().trim_matches(|c| c == '\'' || c == '"');
            if !spec.is_empty() {
                let name_part = spec
                    .split(|c: char| matches!(c, '<' | '>' | '=' | ' '))
                    .next()
                    .unwrap_or(spec)
                    .trim()
                    .trim_end_matches('?');
                if !name_part.is_empty() {
                    dependencies.push(DependencyInfo {
                        name: name_part.to_string(),
                    });
                }
            }
            continue;
        }

        if in_run_depends {
            if trimmed.is_empty() {
                in_run_depends = false;
                continue;
            }

            let first_char = line.chars().next().unwrap_or_default();
            if !first_char.is_whitespace() {
                in_run_depends = false;
                continue;
            }

            if trimmed.contains(':') {
                in_run_depends = false;
                continue;
            }

            let spec = trimmed.trim_matches(|c| c == '\'' || c == '"');
            if spec.is_empty() {
                continue;
            }
            let name_part = spec
                .split(|c: char| matches!(c, '<' | '>' | '=' | ' '))
                .next()
                .unwrap_or(spec)
                .trim()
                .trim_end_matches('?');
            if name_part.is_empty() {
                continue;
            }
            dependencies.push(DependencyInfo {
                name: name_part.to_string(),
            });
        }
    }

    dependencies.sort_by(|a, b| a.name.cmp(&b.name));
    dependencies.dedup_by(|a, b| a.name == b.name);

    Ok(dependencies)
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

fn format_download_size(bytes: u64) -> String {
    const KB: f64 = 1000.0;
    const MB: f64 = KB * 1000.0;
    const GB: f64 = MB * 1000.0;

    if bytes < MB as u64 {
        format!("{:.1} KB", bytes as f64 / KB)
    } else if bytes < GB as u64 {
        format!("{:.2} MB", bytes as f64 / MB)
    } else {
        format!("{:.2} GB", bytes as f64 / GB)
    }
}

fn sanitize_contact_field(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(start) = trimmed.find('<') {
        if let Some(end_rel) = trimmed[start + 1..].find('>') {
            let end = start + 1 + end_rel;
            let name = trimmed[..start].trim();
            let email = trimmed[start + 1..end].trim();

            return match (name.is_empty(), email.is_empty()) {
                (false, false) => format!("{} ({})", name, email),
                (false, true) => name.to_string(),
                (true, false) => email.to_string(),
                (true, true) => String::new(),
            };
        }
    }

    trimmed.replace('<', "").replace('>', "").trim().to_string()
}

fn package_matches_filter(pkg: &PackageInfo, filter_lower: &str) -> bool {
    let needle = filter_lower.trim();
    if needle.is_empty() {
        return true;
    }

    pkg.name_lower.contains(needle)
        || pkg.version_lower.contains(needle)
        || pkg.description_lower.contains(needle)
}

fn run_xbps_query_search(query: &str) -> Result<Vec<PackageInfo>, String> {
    let output = Command::new("xbps-query")
        .args(["-Rs", query])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_query_output(&stdout))
}

fn run_xbps_list_installed() -> Result<Vec<PackageInfo>, String> {
    let output = Command::new("xbps-query")
        .arg("-l")
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_installed_output(&stdout))
}

fn run_xbps_install(package: &str) -> Result<CommandResult, String> {
    run_privileged_command("xbps-install", &["-y", package])
}

fn run_xbps_remove(package: &str) -> Result<CommandResult, String> {
    run_xbps_remove_packages(&[package.to_string()])
}

fn run_xbps_remove_packages(packages: &[String]) -> Result<CommandResult, String> {
    if packages.is_empty() {
        return Ok(CommandResult {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    let mut args = vec!["-y"];
    let package_refs: Vec<&str> = packages.iter().map(|pkg| pkg.as_str()).collect();
    args.extend(package_refs);
    run_privileged_command("xbps-remove", &args)
}

fn query_installed_detail(
    package: &str,
    installed_set: &HashSet<String>,
) -> Result<InstalledDetail, String> {
    let mut detail = InstalledDetail::default();

    match query_pkgsize_bytes(package) {
        Ok(Some(bytes)) => {
            detail.download_bytes = Some(bytes);
            detail.download_formatted = Some(format_download_size(bytes));
        }
        Ok(None) => {}
        Err(err) => detail.download_error = Some(err),
    }

    match run_xbps_query_dependencies(package) {
        Ok(deps) => {
            detail.dependencies = deps
                .into_iter()
                .map(|dep| {
                    let name = dep.name;
                    let installed = installed_set.contains(&name);
                    InstalledDependency { name, installed }
                })
                .collect();
        }
        Err(err) => detail.dependencies_error = Some(err),
    }

    match run_xbps_query_required_by(package) {
        Ok(required) => {
            detail.required_by = required;
        }
        Err(err) => detail.required_by_error = Some(err),
    }

    let metadata = query_package_metadata(package);
    detail.long_description = metadata.long_desc;
    detail.homepage = metadata.homepage;
    detail.maintainer = metadata.maintainer;
    detail.license = metadata.license;

    Ok(detail)
}

fn query_pkgsize_bytes(package: &str) -> Result<Option<u64>, String> {
    if let Some(bytes) = query_size_property(package, "installed_size")? {
        return Ok(Some(bytes));
    }
    query_size_property(package, "pkgsize")
}

fn run_xbps_query_required_by(package: &str) -> Result<Vec<String>, String> {
    let output = Command::new("xbps-query")
        .args(["-X", package])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut required = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (name, _) = split_package_identifier(trimmed);
        if !name.is_empty() {
            required.push(name);
        }
    }
    required.sort();
    required.dedup();
    Ok(required)
}

fn query_size_property(package: &str, property: &str) -> Result<Option<u64>, String> {
    let output = Command::new("xbps-query")
        .args(["-p", property, package])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        let value = trimmed
            .strip_prefix(property)
            .and_then(|s| s.strip_prefix(':'))
            .map(|v| v.trim())
            .unwrap_or(trimmed);
        if let Some(bytes) = parse_bytes_from_field(value) {
            return Ok(Some(bytes));
        }
    }

    Ok(None)
}

fn parse_bytes_from_field(text: &str) -> Option<u64> {
    let trimmed = text.trim().trim_end_matches(|c| c == ',' || c == '.');
    if trimmed.is_empty() {
        return None;
    }

    let cleaned = trimmed.replace(',', "");
    let mut parts = cleaned.split_whitespace();
    if let Some(first) = parts.next() {
        if let Ok(value) = first.parse::<u64>() {
            if let Some(unit) = parts.next() {
                return Some((value as f64 * unit_multiplier(unit)).round() as u64);
            }
            return Some(value);
        }
        if let Ok(value) = first.parse::<f64>() {
            if let Some(unit) = parts.next() {
                return Some((value * unit_multiplier(unit)).round() as u64);
            }
        }
    }

    let mut number = String::new();
    let mut unit = String::new();
    for ch in cleaned.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            number.push(ch);
        } else if !ch.is_whitespace() {
            unit.push(ch);
        }
    }

    if number.is_empty() {
        return None;
    }

    if unit.is_empty() {
        return number.parse::<u64>().ok();
    }

    let value = number.parse::<f64>().ok()?;
    Some((value * unit_multiplier(&unit)).round() as u64)
}

fn unit_multiplier(unit: &str) -> f64 {
    let cleaned = unit
        .trim()
        .trim_matches(|c: char| !c.is_ascii_alphabetic())
        .to_lowercase();
    match cleaned.as_str() {
        "b" | "byte" | "bytes" => 1.0,
        "k" | "kb" | "kib" | "ki" => 1024.0,
        "m" | "mb" | "mib" | "mi" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" | "gi" => 1024.0 * 1024.0 * 1024.0,
        "t" | "tb" | "tib" | "ti" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        other if other.ends_with("ib") => match &other[..other.len() - 2] {
            "k" => 1024.0,
            "m" => 1024.0 * 1024.0,
            "g" => 1024.0 * 1024.0 * 1024.0,
            "t" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
            _ => 1.0,
        },
        _ => 1.0,
    }
}

#[derive(Default)]
struct PackageMetadata {
    long_desc: Option<String>,
    homepage: Option<String>,
    maintainer: Option<String>,
    license: Option<String>,
    repository: Option<String>,
}

fn query_package_metadata(package: &str) -> PackageMetadata {
    const PROPERTIES: [&str; 5] = [
        "long_desc",
        "homepage",
        "maintainer",
        "license",
        "repository",
    ];
    let mut metadata = PackageMetadata::default();

    if let Some(values) = query_properties_bulk(package, &PROPERTIES) {
        if let Some(long_desc) = values.get("long_desc").and_then(parse_long_description) {
            metadata.long_desc = Some(long_desc);
        }
        metadata.homepage = values.get("homepage").and_then(clean_simple_property);
        metadata.maintainer = values.get("maintainer").and_then(clean_simple_property);
        metadata.license = values.get("license").and_then(clean_simple_property);
        metadata.repository = values.get("repository").and_then(clean_simple_property);
    }

    metadata
}

fn parse_long_description(raw: &String) -> Option<String> {
    let mut lines = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        lines.push(trimmed.to_string());
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn clean_simple_property(raw: &String) -> Option<String> {
    let trimmed = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    if trimmed.is_empty() || trimmed == "-" {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn query_properties_bulk(package: &str, properties: &[&str]) -> Option<HashMap<String, String>> {
    if properties.is_empty() {
        return Some(HashMap::new());
    }

    let mut command = Command::new("xbps-query");
    for prop in properties {
        command.arg("-p");
        command.arg(prop);
    }
    command.arg(package);

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let property_set: HashSet<&str> = properties.iter().copied().collect();
    let mut result: HashMap<String, String> = HashMap::new();

    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for line in stdout.lines() {
        let trimmed_end = line.trim_end();
        if trimmed_end.is_empty() {
            continue;
        }

        if let Some((candidate, remainder)) = trimmed_end.split_once(':') {
            let key = candidate.trim();
            if property_set.contains(key) {
                if let Some(prev_key) = current_key.take() {
                    let normalized = normalize_property_text(&current_value);
                    result.entry(prev_key).or_insert(normalized);
                }
                current_key = Some(key.to_string());
                current_value = remainder.trim().to_string();
                continue;
            }
        }

        if let Some(_) = current_key {
            let value = trimmed_end.trim();
            if value.is_empty() {
                continue;
            }
            if !current_value.is_empty() {
                current_value.push('\n');
            }
            current_value.push_str(value);
            continue;
        }

        if properties.len() == 1 {
            let key = properties[0].to_string();
            let normalized = normalize_property_text(trimmed_end);
            result.entry(key).or_insert(normalized);
        }
    }

    if let Some(prev_key) = current_key {
        let normalized = normalize_property_text(&current_value);
        result.entry(prev_key).or_insert(normalized);
    }

    Some(result)
}

fn normalize_property_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        String::new()
    } else if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn run_privileged_command(program: &str, args: &[&str]) -> Result<CommandResult, String> {
    let output = Command::new("pkexec")
        .arg(program)
        .args(args)
        .output()
        .map_err(|err| format!("Failed to launch pkexec: {}", err))?;

    Ok(CommandResult {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn run_xbps_check_updates() -> Result<Vec<PackageInfo>, String> {
    let output = Command::new("xbps-install")
        .args(["-Sun"])
        .env("NO_COLOR", "1")
        .env("XBPS_INSTALL_VERBOSE", "2")
        .output()
        .map_err(|err| format!("Failed to launch xbps-install: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let cleaned = strip_ansi_codes(&stdout);
    Ok(parse_updates_output(&cleaned))
}

fn run_xbps_update_all() -> Result<CommandResult, String> {
    run_privileged_command("xbps-install", &["-y", "-Su"])
}

fn run_xbps_update_package(package: &str) -> Result<CommandResult, String> {
    run_privileged_command("xbps-install", &["-y", "-u", package])
}

fn run_xbps_update_packages(packages: &[String]) -> Result<CommandResult, String> {
    if packages.is_empty() {
        return Ok(CommandResult {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    let mut args = vec!["-y", "-u"];
    let package_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
    args.extend(package_refs);
    run_privileged_command("xbps-install", &args)
}

fn query_repo_package_info(name: &str) -> Result<PackageInfo, String> {
    let output = Command::new("xbps-query")
        .args(["-R", name])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut pkgver = String::new();
    let mut description = String::new();
    let mut pkgsize_bytes: Option<u64> = None;
    let mut download_literal: Option<String> = None;
    let mut changelog: Option<String> = None;
    let mut capture_changelog = false;

    for line in stdout.lines() {
        if let Some(value) = line.strip_prefix("pkgver:") {
            pkgver = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("short_desc:") {
            description = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("pkgsize:") {
            let trimmed = value.trim();
            if pkgsize_bytes.is_none() {
                pkgsize_bytes = parse_bytes_from_field(trimmed).or_else(|| parse_bytes(trimmed));
            }
            if download_literal.is_none() && !trimmed.is_empty() {
                download_literal = Some(trimmed.to_string());
            }
        } else if let Some(value) = line.strip_prefix("filename-size:") {
            let trimmed = value.trim();
            if pkgsize_bytes.is_none() {
                pkgsize_bytes = parse_bytes_from_field(trimmed).or_else(|| parse_bytes(trimmed));
            }
            if download_literal.is_none() && !trimmed.is_empty() {
                download_literal = Some(trimmed.to_string());
            }
        } else if let Some(value) = line.strip_prefix("changelog:") {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                capture_changelog = true;
            } else {
                changelog = Some(trimmed.to_string());
            }
        } else if capture_changelog {
            if line.starts_with(' ') || line.starts_with('\t') {
                let trimmed = line.trim();
                if !trimmed.is_empty() && changelog.is_none() {
                    changelog = Some(trimmed.to_string());
                }
            }
            capture_changelog = false;
        }
    }

    if description.is_empty() {
        description = "Update available".to_string();
    }

    let version = if !pkgver.is_empty() {
        let (_, ver) = split_package_identifier(&pkgver);
        ver
    } else {
        String::new()
    };

    let download_bytes = pkgsize_bytes;
    let download_size = download_bytes.map(format_size).or(download_literal);

    let name_owned = name.to_string();
    let version_lower = lowercase_cache(&version);
    let description_lower = lowercase_cache(&description);

    Ok(PackageInfo {
        name_lower: lowercase_cache(&name_owned),
        version_lower,
        description_lower,
        name: name_owned,
        version,
        description,
        installed: true,
        previous_version: None,
        download_size,
        changelog,
        download_bytes,
        repository: None,
        build_date: None,
        first_seen: None,
    })
}

fn query_installed_package_version(name: &str) -> Option<String> {
    let output = Command::new("xbps-query")
        .args(["-p", "pkgver", name])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let identifier = trimmed
            .strip_prefix("pkgver:")
            .map(|value| value.trim())
            .unwrap_or(trimmed);
        let (_name, version) = split_package_identifier(identifier);
        if !version.is_empty() {
            return Some(version);
        }
    }

    None
}

fn query_discover_detail(package: &str) -> Result<DiscoverDetail, String> {
    let info = query_repo_package_info(package)?;
    let PackageMetadata {
        long_desc,
        homepage,
        maintainer,
        license,
        repository,
    } = query_package_metadata(package);

    let description = if let Some(desc) = long_desc {
        Some(desc)
    } else if info.description.is_empty() {
        None
    } else {
        Some(info.description.clone())
    };

    let version = if info.version.is_empty() {
        None
    } else {
        Some(info.version.clone())
    };

    let download_bytes = info.download_bytes;
    let download = info
        .download_size
        .clone()
        .or_else(|| download_bytes.map(format_size));

    let dependencies = run_xbps_query_dependencies(package).unwrap_or_default();

    Ok(DiscoverDetail {
        version,
        description,
        download,
        download_bytes,
        homepage,
        maintainer,
        license,
        repository,
        dependencies,
    })
}

fn detail_download_bytes(package: &str) -> Option<u64> {
    query_pkgsize_bytes(package).ok().unwrap_or(None)
}

fn parse_updates_output(text: &str) -> Vec<PackageInfo> {
    let mut updates = Vec::new();

    for raw_line in text.lines() {
        let mut line = raw_line.trim().trim_start_matches('\r');
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("xbps-install:") {
            line = rest.trim();
        }

        line = line
            .trim_start_matches(|c: char| c == '*' || c == '-' || c == '>')
            .trim();

        loop {
            if line.starts_with('[') {
                if let Some(pos) = line.find(']') {
                    line = line[pos + 1..].trim();
                    continue;
                }
            }
            break;
        }

        if line.is_empty() {
            continue;
        }

        // Pattern: name-version -> newversion
        if let Some(idx) = line.find("->") {
            let left = line[..idx].trim();
            let right = line[idx + 2..].trim();
            if left.is_empty() || right.is_empty() {
                continue;
            }

            let (name, prev_version) = split_package_identifier(left);
            if name.is_empty() {
                continue;
            }

            let new_version_token = right
                .split_whitespace()
                .find(|token| token.chars().any(|c| c.is_ascii_digit()))
                .unwrap_or("");
            let version = if new_version_token.contains('-') {
                let (_, ver) = split_package_identifier(new_version_token);
                ver
            } else {
                new_version_token.to_string()
            };

            add_update_entry(&mut updates, name, version, Some(prev_version));
            continue;
        }

        // Pattern: name-version update available (installed: ...)
        if line.contains("update available") {
            let (identifier, rest) = match line.split_once(" update available") {
                Some((id, remainder)) => (id.trim(), remainder.trim()),
                None => continue,
            };
            if identifier.is_empty() {
                continue;
            }

            let (name, version) = split_package_identifier(identifier);
            let previous_version = rest
                .split("(installed:")
                .nth(1)
                .and_then(|segment| segment.split(')').next())
                .map(|text| text.trim().to_string());

            add_update_entry(&mut updates, name, version, previous_version);
            continue;
        }

        // Generic "update" text, e.g. "pkg-1.0_1 update (pkg-1.0_2)"
        if let Some(idx) = line.find(" update") {
            let left = line[..idx].trim();
            let right = line[idx + " update".len()..].trim();
            if left.is_empty() {
                continue;
            }

            let (name, prev_version) = split_package_identifier(left);
            if name.is_empty() {
                continue;
            }

            let new_version_token = right
                .split(|c| c == ')' || c == ' ' || c == ',' || c == ':')
                .find(|part| part.contains('-') || part.chars().any(|c| c.is_ascii_digit()))
                .unwrap_or("")
                .trim_start_matches('(')
                .trim();

            let version = if new_version_token.contains('-') {
                let (_, ver) = split_package_identifier(new_version_token);
                ver
            } else {
                new_version_token.to_string()
            };

            add_update_entry(&mut updates, name, version, Some(prev_version));
        }
    }

    updates.sort_by(|a, b| a.name.cmp(&b.name));
    updates
}

fn add_update_entry(
    updates: &mut Vec<PackageInfo>,
    name: String,
    version: String,
    previous_version: Option<String>,
) {
    if name.is_empty() {
        return;
    }

    let mut info = query_repo_package_info(&name).unwrap_or_else(|_| {
        let description = "Update available".to_string();
        PackageInfo {
            name_lower: lowercase_cache(&name),
            version_lower: lowercase_cache(&version),
            description_lower: lowercase_cache(&description),
            name: name.clone(),
            version: version.clone(),
            description,
            installed: true,
            previous_version: previous_version.clone(),
            download_size: None,
            changelog: None,
            download_bytes: None,
            repository: None,
            build_date: None,
            first_seen: None,
        }
    });

    if looks_like_version(&version) {
        info.set_version(version);
    }
    info.installed = true;
    if let Some(installed) = query_installed_package_version(&name) {
        info.previous_version = Some(installed);
    } else if let Some(prev) = previous_version {
        if !prev.is_empty() {
            info.previous_version = Some(prev);
        }
    }
    if info.download_size.is_none() {
        if let Some(bytes) = info.download_bytes {
            info.download_size = Some(format_size(bytes));
        }
    }

    updates.push(info);
}

fn looks_like_version(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let mut chars = text.chars();
    if let Some(first) = chars.next() {
        if first.is_ascii_digit() {
            return true;
        }
        if matches!(first, 'v' | 'r') {
            return chars.next().map(|c| c.is_ascii_digit()).unwrap_or(false);
        }
    }

    false
}

fn strip_ansi_codes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            continue;
        }
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                while let Some(next) = chars.next() {
                    if (next >= 'a' && next <= 'z') || (next >= 'A' && next <= 'Z') {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn parse_bytes(text: &str) -> Option<u64> {
    let cleaned = text
        .split_whitespace()
        .next()
        .unwrap_or(text)
        .trim()
        .trim_end_matches(|c: char| c == ',' || c == '.');
    cleaned.parse().ok()
}

fn parse_query_output(output: &str) -> Vec<PackageInfo> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            let mut tokens = trimmed.split_whitespace();
            let first = tokens.next()?;

            let (marker, identifier_token) = if first.starts_with('[') && first.ends_with(']') {
                (Some(first), tokens.next()?)
            } else {
                (None, first)
            };

            let mut installed = false;
            if let Some(marker) = marker {
                installed = marker.contains('x') || marker.contains('X');
            }

            let identifier = identifier_token.trim();
            let rest = tokens.collect::<Vec<_>>().join(" ");
            let (name, version) = split_package_identifier(identifier);

            let description = rest;
            Some(PackageInfo {
                name_lower: lowercase_cache(&name),
                version_lower: lowercase_cache(&version),
                description_lower: lowercase_cache(&description),
                name,
                version,
                description,
                installed,
                previous_version: None,
                download_size: None,
                changelog: None,
                download_bytes: None,
                repository: None,
                build_date: None,
                first_seen: None,
            })
        })
        .collect()
}

fn parse_installed_output(output: &str) -> Vec<PackageInfo> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            let mut split = trimmed.split_whitespace();
            let _status = split.next()?;
            let identifier = split.next()?;
            let (name, version) = split_package_identifier(identifier);

            let description_index = trimmed.find(identifier).map(|idx| idx + identifier.len());
            let description = description_index
                .and_then(|pos| trimmed.get(pos..))
                .map(|rest| rest.trim().to_string())
                .unwrap_or_default();

            Some(PackageInfo {
                name_lower: lowercase_cache(&name),
                version_lower: lowercase_cache(&version),
                description_lower: lowercase_cache(&description),
                name,
                version,
                description,
                installed: true,
                previous_version: None,
                download_size: None,
                changelog: None,
                download_bytes: None,
                repository: None,
                build_date: None,
                first_seen: None,
            })
        })
        .collect()
}

fn split_package_identifier(identifier: &str) -> (String, String) {
    if let Some(pos) = identifier.rfind('-') {
        let (name, version_part) = identifier.split_at(pos);
        (
            name.to_string(),
            version_part.trim_start_matches('-').to_string(),
        )
    } else {
        (identifier.to_string(), String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_remote_spotlight_metadata_returns_packages() {
        let packages = fetch_remote_spotlight_metadata().expect("fetch spotlight metadata");
        assert!(
            !packages.is_empty(),
            "expected spotlight metadata to include packages"
        );
    }

    #[test]
    fn refresh_spotlight_cache_produces_spotlight_lists() {
        let cache = SpotlightCache::default();
        let outcome = refresh_spotlight_cache(cache).expect("refresh spotlight cache");
        assert!(
            !outcome.recent.is_empty(),
            "expected recent spotlight entries"
        );
        assert!(
            !outcome.categories.is_empty(),
            "expected spotlight categories"
        );
    }
}
