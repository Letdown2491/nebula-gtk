mod categories;
mod details;
mod helpers;
mod settings;
mod spotlight;
mod state;
mod types;
mod ui;
mod xbps;

use adw::prelude::*;
use gtk4::gio;
use gtk4::glib;
use libadwaita as adw;

use crate::ui::build_ui;

const APP_ID: &str = "tech.geektoshi.Nebula";

fn main() -> glib::ExitCode {
    adw::init().expect("Failed to initialize libadwaita");

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::FLAGS_NONE)
        .build();

    app.connect_activate(build_ui);

    app.run()
}
