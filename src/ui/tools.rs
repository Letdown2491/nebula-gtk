use gtk::pango;
use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;


#[derive(Clone)]
pub(crate) struct ToolsWidgets {
    pub(crate) cleanup_button: gtk::Button,
    pub(crate) cleanup_spinner: gtk::Spinner,
    pub(crate) cache_clean_button: gtk::Button,
    pub(crate) cache_clean_spinner: gtk::Spinner,
    pub(crate) cache_clean_spin_button: gtk::SpinButton,
    pub(crate) pkgdb_button: gtk::Button,
    pub(crate) pkgdb_spinner: gtk::Spinner,
    pub(crate) reconfigure_button: gtk::Button,
    pub(crate) reconfigure_spinner: gtk::Spinner,
    pub(crate) alternatives_button: gtk::Button,
    pub(crate) alternatives_spinner: gtk::Spinner,
    pub(crate) status_label: gtk::Label,
    pub(crate) status_revealer: gtk::Revealer,
}

pub(crate) fn build_page() -> (gtk::Box, ToolsWidgets) {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_start(16)
        .margin_end(16)
        .margin_top(0)
        .margin_bottom(16)
        .build();
    container.set_vexpand(true);
    container.set_hexpand(true);

    let clamp = adw::Clamp::builder()
        .maximum_size(820)
        .tightening_threshold(540)
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(32)
        .margin_bottom(32)
        .margin_start(32)
        .margin_end(32)
        .build();

    clamp.set_child(Some(&content));
    container.append(&clamp);

    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .build();

    let title_label = gtk::Label::builder()
        .label("Tools for package maintenance")
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    title_label.add_css_class("title-2");

    let subtitle_label = gtk::Label::builder()
        .label(
            "This page provides easy access to XBPS tooling for package cleaning and maintenance.",
        )
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    subtitle_label.add_css_class("dim-label");

    header_box.append(&title_label);
    header_box.append(&subtitle_label);
    content.append(&header_box);

    let quick_group = adw::PreferencesGroup::builder()
        .title("Package Cleanup")
        .description("Cleanup utilities to keep old packages from clogging up your system.")
        .build();

    let (cleanup_row, cleanup_button, cleanup_spinner) = build_tools_action_row(
        "Remove orphaned packages",
        "Clean out unused dependencies.",
        "Run cleanup",
        "Runs \"xbps-remove -O\" to prune orphaned packages.",
    );
    quick_group.add(&cleanup_row);

    // Cache clean row with SpinButton for keeping N versions
    let cache_clean_row = adw::ActionRow::builder()
        .title("Clean package cache")
        .subtitle("Remove old package versions from cache.")
        .build();
    cache_clean_row.set_activatable(false);

    let cache_clean_spinner = gtk::Spinner::new();
    cache_clean_spinner.set_visible(false);
    cache_clean_spinner.set_valign(gtk::Align::Center);
    cache_clean_spinner.set_size_request(16, 16);

    // SpinButton for keeping N versions (1-5)
    let cache_clean_adjustment = gtk::Adjustment::new(1.0, 1.0, 5.0, 1.0, 1.0, 0.0);
    let cache_clean_spin_button = gtk::SpinButton::builder()
        .adjustment(&cache_clean_adjustment)
        .valign(gtk::Align::Center)
        .width_chars(2)
        .build();
    cache_clean_spin_button.set_tooltip_text(Some("Number of package versions to keep in cache"));

    let keep_label = gtk::Label::builder()
        .label("Keep:")
        .valign(gtk::Align::Center)
        .build();
    keep_label.add_css_class("dim-label");

    let cache_clean_button = gtk::Button::builder()
        .label("Clean cache")
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    cache_clean_button.set_focus_on_click(false);
    cache_clean_button.set_tooltip_text(Some("Clean package cache, keeping the selected number of versions"));

    let cache_controls = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    cache_controls.append(&keep_label);
    cache_controls.append(&cache_clean_spin_button);
    cache_controls.append(&cache_clean_spinner);
    cache_controls.append(&cache_clean_button);
    cache_clean_row.add_suffix(&cache_controls);

    quick_group.add(&cache_clean_row);

    content.append(&quick_group);

    let repair_group = adw::PreferencesGroup::builder()
        .title("Repair &amp; recovery")
        .description(
            "Reach for these when installs act strangely or the system had a rough shutdown.",
        )
        .build();

    let (pkgdb_row, pkgdb_button, pkgdb_spinner) = build_tools_action_row(
        "Verify package database",
        "Checks and repairs package metadata. Handy after forced power-offs.",
        "Run verification",
        "Runs \"xbps-pkgdb -a\" to verify package metadata.",
    );
    repair_group.add(&pkgdb_row);

    let (reconfigure_row, reconfigure_button, reconfigure_spinner) =
        build_tools_action_row(
            "Reconfigure everything",
            "Replays post-install hooks for every package. Give it time to finish.",
            "Run reconfigure",
            "Runs \"xbps-reconfigure -a\" to re-run post-install hooks.",
        );
    repair_group.add(&reconfigure_row);
    content.append(&repair_group);

    let alternatives_group = adw::PreferencesGroup::builder()
        .title("Alternatives")
        .description("See which providers are currently registered before you switch defaults.")
        .build();

    let (alternatives_row, alternatives_button, alternatives_spinner) =
        build_tools_action_row(
            "List available alternatives",
            "Shows every registered provider so you know what is installed.",
            "Show list",
            "Runs \"xbps-alternatives -l\" and displays the output.",
        );
    alternatives_group.add(&alternatives_row);
    content.append(&alternatives_group);

    // Footer status area
    let status_revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideUp)
        .transition_duration(200)
        .reveal_child(false)
        .build();

    let status_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::Center)
        .spacing(8)
        .margin_top(16)
        .margin_bottom(8)
        .margin_start(32)
        .margin_end(32)
        .build();
    status_box.add_css_class("toolbar");

    let status_label = gtk::Label::builder()
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .halign(gtk::Align::Center)
        .build();

    status_box.append(&status_label);
    status_revealer.set_child(Some(&status_box));
    content.append(&status_revealer);

    let widgets = ToolsWidgets {
        cleanup_button,
        cleanup_spinner,
        cache_clean_button,
        cache_clean_spinner,
        cache_clean_spin_button,
        pkgdb_button,
        pkgdb_spinner,
        reconfigure_button,
        reconfigure_spinner,
        alternatives_button,
        alternatives_spinner,
        status_label,
        status_revealer,
    };

    (container, widgets)
}

fn build_tools_action_row(
    title: &str,
    blurb: &str,
    button_label: &str,
    tooltip: &str,
) -> (adw::ActionRow, gtk::Button, gtk::Spinner) {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(blurb)
        .build();
    row.set_activatable(false);

    let spinner = gtk::Spinner::new();
    spinner.set_visible(false);
    spinner.set_valign(gtk::Align::Center);
    spinner.set_size_request(16, 16);

    let button = gtk::Button::builder()
        .label(button_label)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    button.set_focus_on_click(false);
    if !tooltip.is_empty() {
        button.set_tooltip_text(Some(tooltip));
    }

    let controls = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .halign(gtk::Align::End)
        .valign(gtk::Align::Center)
        .build();
    controls.append(&spinner);
    controls.append(&button);
    row.add_suffix(&controls);

    (row, button, spinner)
}
