use gtk::pango;
use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;

use crate::state::controller::tools::{MaintenanceTask, maintenance_copy};

#[derive(Clone)]
pub(crate) struct ToolsWidgets {
    pub(crate) cleanup_button: gtk::Button,
    pub(crate) cleanup_spinner: gtk::Spinner,
    pub(crate) cleanup_status: gtk::Label,
    pub(crate) pkgdb_button: gtk::Button,
    pub(crate) pkgdb_spinner: gtk::Spinner,
    pub(crate) pkgdb_status: gtk::Label,
    pub(crate) reconfigure_button: gtk::Button,
    pub(crate) reconfigure_spinner: gtk::Spinner,
    pub(crate) reconfigure_status: gtk::Label,
    pub(crate) alternatives_button: gtk::Button,
    pub(crate) alternatives_spinner: gtk::Spinner,
    pub(crate) alternatives_status: gtk::Label,
}

pub(crate) fn build_page() -> (gtk::Box, ToolsWidgets) {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
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
        .label("Keep things running smoothly")
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    title_label.add_css_class("title-2");

    let subtitle_label = gtk::Label::builder()
        .label("A handful of maintenance helpers for the moments when Nebula needs a bit of attention.")
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
        .title("Quick tidy-up")
        .description("Short chores that keep Void Linux neat without getting in the way.")
        .build();

    let (cleanup_panel, cleanup_button, cleanup_status, cleanup_spinner) = build_tools_action_row(
        "Remove orphaned packages",
        "Sweep out dependencies nothing else needs.",
        "Run cleanup",
        "Runs \"xbps-remove -O\" to prune orphaned packages.",
        maintenance_copy(MaintenanceTask::Cleanup).idle_text,
    );
    quick_group.add(&cleanup_panel);
    content.append(&quick_group);

    let repair_group = adw::PreferencesGroup::builder()
        .title("Repair &amp; recovery")
        .description(
            "Reach for these when installs act strangely or the system had a rough shutdown.",
        )
        .build();

    let (pkgdb_panel, pkgdb_button, pkgdb_status, pkgdb_spinner) = build_tools_action_row(
        "Verify package database",
        "Checks and repairs package metadata. Handy after forced power-offs.",
        "Run verification",
        "Runs \"xbps-pkgdb -a\" to verify package metadata.",
        maintenance_copy(MaintenanceTask::Pkgdb).idle_text,
    );
    repair_group.add(&pkgdb_panel);

    let (reconfigure_panel, reconfigure_button, reconfigure_status, reconfigure_spinner) =
        build_tools_action_row(
            "Reconfigure everything",
            "Replays post-install hooks for every package. Give it time to finish.",
            "Run reconfigure",
            "Runs \"xbps-reconfigure -a\" to re-run post-install hooks.",
            maintenance_copy(MaintenanceTask::Reconfigure).idle_text,
        );
    repair_group.add(&reconfigure_panel);
    content.append(&repair_group);

    let alternatives_group = adw::PreferencesGroup::builder()
        .title("Alternatives")
        .description("Peek at which providers are currently registered before you switch defaults.")
        .build();

    let (alternatives_panel, alternatives_button, alternatives_status, alternatives_spinner) =
        build_tools_action_row(
            "List available alternatives",
            "Shows every registered provider so you know what is installed.",
            "Show list",
            "Runs \"xbps-alternatives -l\" and displays the output.",
            maintenance_copy(MaintenanceTask::Alternatives).idle_text,
        );
    alternatives_group.add(&alternatives_panel);
    content.append(&alternatives_group);

    let widgets = ToolsWidgets {
        cleanup_button,
        cleanup_spinner,
        cleanup_status,
        pkgdb_button,
        pkgdb_spinner,
        pkgdb_status,
        reconfigure_button,
        reconfigure_spinner,
        reconfigure_status,
        alternatives_button,
        alternatives_spinner,
        alternatives_status,
    };

    (container, widgets)
}

fn build_tools_action_row(
    title: &str,
    blurb: &str,
    button_label: &str,
    tooltip: &str,
    initial_status: &str,
) -> (gtk::Box, gtk::Button, gtk::Label, gtk::Spinner) {
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

    let status_label = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    status_label.add_css_class("caption");
    status_label.add_css_class("dim-label");
    status_label.set_text(initial_status);

    let wrapper = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();
    wrapper.append(&row);
    wrapper.append(&status_label);

    (wrapper, button, status_label, spinner)
}
