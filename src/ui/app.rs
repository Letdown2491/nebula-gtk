use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use glib::{Variant, VariantTy};
use gtk::{gdk, gio, glib};

use crate::settings::{ThemePreference, load_app_settings, save_app_settings};
use crate::state::controller::AppController;
use crate::state::types::AppMessage;
use crate::ui::{
    DiscoverWidgets, InstalledWidgets, ThemeGlyph, ToolsWidgets, UpdatesWidgets,
    apply_theme_css_class, build_discover_page, build_installed_page, build_theme_icon,
    build_tools_page, build_updates_page,
};

pub(crate) struct AppWidgets {
    pub(crate) toast_overlay: adw::ToastOverlay,
    pub(crate) view_stack: adw::ViewStack,
    pub(crate) discover: DiscoverWidgets,
    pub(crate) installed: InstalledWidgets,
    pub(crate) updates: UpdatesWidgets,
    pub(crate) tools: ToolsWidgets,
    pub(crate) updates_page: adw::ViewStackPage,
}

pub(crate) fn build_ui(app: &adw::Application) {
    #[cfg(not(nebula_skip_gresource))]
    gio::resources_register_include!("nebula.gresource")
        .expect("Failed to register embedded resources");

    #[cfg(nebula_skip_gresource)]
    {
        eprintln!("Nebula running without embedded resources (SKIP_GRESOURCE=1)");
    }
    if let Some(display) = gdk::Display::default() {
        let theme = gtk::IconTheme::for_display(&display);
        theme.add_resource_path("/tech/geektoshi/Nebula/icons");
    }

    let settings = Rc::new(RefCell::new(load_app_settings()));
    let (initial_width, initial_height) = {
        let settings = settings.borrow();
        (
            settings.window_width.unwrap_or(1080),
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
    view_stack.set_vexpand(true);
    view_stack.set_hexpand(true);

    let header_bar = adw::HeaderBar::new();
    header_bar.add_css_class("nebula-headerbar");
    header_bar.set_hexpand(true);
    let view_switcher = adw::ViewSwitcher::new();
    view_switcher.set_stack(Some(&view_stack));
    view_switcher.set_halign(gtk::Align::Center);
    header_bar.set_title_widget(Some(&view_switcher));
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
    style_manager.connect_dark_notify(glib::clone!(
        #[weak]
        window,
        move |manager| {
            apply_theme_css_class(&window, manager.is_dark());
        },
    ));

    let theme_action = gio::SimpleAction::new_stateful(
        "theme",
        Some(&VariantTy::STRING),
        &Variant::from(current_theme.as_str()),
    );
    theme_action.connect_change_state(glib::clone!(
        #[weak]
        style_manager,
        #[strong]
        settings,
        move |action, value| {
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
        },
    ));
    app.add_action(&theme_action);

    let preferences_action = gio::SimpleAction::new("preferences", None);
    app.add_action(&preferences_action);

    let mirrors_action = gio::SimpleAction::new("mirrors", None);
    app.add_action(&mirrors_action);

    let show_updates_action = gio::SimpleAction::new("show-updates", None);
    app.add_action(&show_updates_action);

    let about_action = gio::SimpleAction::new("about", None);
    app.add_action(&about_action);

    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    let popover = gtk::Popover::new();
    menu_button.set_popover(Some(&popover));

    let popover_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    let theme_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();

    let system_button = gtk::Button::builder().has_frame(false).build();
    system_button.add_css_class("flat");
    system_button.set_child(Some(&build_theme_icon(ThemeGlyph::System)));
    system_button.set_tooltip_text(Some("Match system theme"));

    let light_button = gtk::Button::builder().has_frame(false).build();
    light_button.add_css_class("flat");
    light_button.set_child(Some(&build_theme_icon(ThemeGlyph::Light)));
    light_button.set_tooltip_text(Some("Use light theme"));

    let dark_button = gtk::Button::builder().has_frame(false).build();
    dark_button.add_css_class("flat");
    dark_button.set_child(Some(&build_theme_icon(ThemeGlyph::Dark)));
    dark_button.set_tooltip_text(Some("Use dark theme"));

    let theme_buttons = vec![
        ("system".to_string(), system_button.clone()),
        ("light".to_string(), light_button.clone()),
        ("dark".to_string(), dark_button.clone()),
    ];
    for (key, button) in theme_buttons.iter() {
        let action = theme_action.clone();
        let popover = popover.clone();
        let key = key.clone();
        button.connect_clicked(glib::clone!(
            #[weak]
            popover,
            move |_| {
                action.activate(Some(&Variant::from(key.as_str())));
                popover.popdown();
            },
        ));
    }

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
    style_manager.connect_color_scheme_notify(glib::clone!(
        #[strong]
        theme_buttons_rc,
        move |manager| {
            refresh_theme_buttons(manager.color_scheme(), &theme_buttons_rc);
        },
    ));

    theme_box.append(&system_button);
    theme_box.append(&light_button);
    theme_box.append(&dark_button);

    let theme_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    theme_list.add_css_class("boxed-list");

    let theme_row = adw::ActionRow::builder().title("Switch theme").build();
    theme_row.add_suffix(&theme_box);
    theme_row.set_activatable(false);
    theme_list.append(&theme_row);

    popover_box.append(&theme_list);

    let menu_list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .build();
    menu_list.add_css_class("boxed-list");

    let prefs_row = adw::ActionRow::builder()
        .title("Preferences")
        .activatable(true)
        .build();
    prefs_row.set_action_name(Some("app.preferences"));
    menu_list.append(&prefs_row);

    let mirrors_row = adw::ActionRow::builder()
        .title("Mirrors")
        .activatable(true)
        .build();
    mirrors_row.set_action_name(Some("app.mirrors"));
    menu_list.append(&mirrors_row);

    let about_row = adw::ActionRow::builder()
        .title("About Nebula")
        .activatable(true)
        .build();
    about_row.set_action_name(Some("app.about"));
    menu_list.append(&about_row);

    popover_box.append(&menu_list);

    popover.set_child(Some(&popover_box));
    menu_button.set_popover(Some(&popover));

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
    let (tools_page, tools_widgets) = build_tools_page();

    {
        let page = view_stack.add_titled(&discover_page, Some("discover"), "Discover");
        page.set_icon_name(Some("discover"));
    }
    {
        let page = view_stack.add_titled(&installed_page, Some("installed"), "Installed");
        page.set_icon_name(Some("installed"));
    }
    let updates_page_ref = view_stack.add_titled(&updates_page, Some("updates"), "Updates");
    updates_page_ref.set_icon_name(Some("updates"));
    {
        let page = view_stack.add_titled(&tools_page, Some("tools"), "Tools");
        page.set_icon_name(Some("tools"));
    }
    updates_page_ref.set_badge_number(0);
    content.append(&view_stack);

    let widgets = AppWidgets {
        toast_overlay: toast_overlay.clone(),
        view_stack: view_stack.clone(),
        discover: discover_widgets,
        installed: installed_widgets,
        updates: updates_widgets,
        tools: tools_widgets,
        updates_page: updates_page_ref,
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
    controller.initialize_mirrors();

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
        let popover_clone = popover.clone();
        preferences_action.connect_activate(move |_, _| {
            popover_clone.popdown();
            if let Some(controller) = controller_weak.upgrade() {
                controller.show_preferences();
            }
        });
    }

    {
        let controller_weak = Rc::downgrade(&controller);
        let popover_clone = popover.clone();
        mirrors_action.connect_activate(move |_, _| {
            popover_clone.popdown();
            if let Some(controller) = controller_weak.upgrade() {
                controller.show_mirrors();
            }
        });
    }

    {
        let controller_weak = Rc::downgrade(&controller);
        let popover_clone = popover.clone();
        about_action.connect_activate(move |_, _| {
            popover_clone.popdown();
            if let Some(controller) = controller_weak.upgrade() {
                controller.show_about_dialog();
            }
        });
    }

    controller.initialize_spotlight();
    controller.refresh_installed_packages();
    {
        let controller_weak = Rc::downgrade(&controller);
        glib::idle_add_local(move || {
            if let Some(controller) = controller_weak.upgrade() {
                controller.refresh_updates(true);
            }
            glib::ControlFlow::Break
        });
    }

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
