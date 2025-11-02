use gtk::pango;
use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;

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

pub(crate) struct DiscoverWidgets {
    pub(crate) search_entry: gtk::SearchEntry,
    pub(crate) search_button: gtk::Button,
    pub(crate) search_spinner: gtk::Spinner,
    pub(crate) status_label: gtk::Label,
    pub(crate) list: gtk::ListBox,
    pub(crate) scroller: gtk::ScrolledWindow,
    pub(crate) content_row: gtk::Box,
    pub(crate) detail_stack: gtk::Stack,
    pub(crate) detail_name: gtk::Label,
    pub(crate) detail_back_button: gtk::Button,
    pub(crate) detail_close_button: gtk::Button,
    pub(crate) detail_version_value: gtk::Label,
    pub(crate) detail_repository_row: gtk::Box,
    pub(crate) detail_repository_value: gtk::Label,
    pub(crate) detail_description: gtk::Label,
    pub(crate) detail_download_value: gtk::Label,
    pub(crate) detail_homepage_row: gtk::Box,
    pub(crate) detail_homepage_link: gtk::LinkButton,
    pub(crate) detail_maintainer_row: gtk::Box,
    pub(crate) detail_maintainer_value: gtk::Label,
    pub(crate) detail_license_row: gtk::Box,
    pub(crate) detail_license_value: gtk::Label,
    pub(crate) detail_update_label: gtk::Label,
    pub(crate) detail_action_button: gtk::Button,
    pub(crate) detail_dependencies_stack: gtk::Stack,
    pub(crate) detail_dependencies_list: gtk::ListBox,
    pub(crate) detail_dependencies_placeholder: gtk::Label,
    pub(crate) detail_frame: gtk::Frame,
    pub(crate) spotlight_spinner: gtk::Spinner,
    pub(crate) spotlight_status: gtk::Label,
    pub(crate) spotlight_recent_stack: gtk::Stack,
    pub(crate) spotlight_recent_list: gtk::ListBox,
    pub(crate) spotlight_recent_scroller: gtk::ScrolledWindow,
    pub(crate) spotlight_recent_detail_revealer: gtk::Revealer,
    pub(crate) spotlight_recent_detail_container: gtk::Box,
    pub(crate) spotlight_recent_back_button: gtk::Button,
    pub(crate) spotlight_recent_detail_name: gtk::Label,
    pub(crate) spotlight_recent_detail_spinner: gtk::Spinner,
    pub(crate) spotlight_recent_detail_version_value: gtk::Label,
    pub(crate) spotlight_recent_detail_repo_row: gtk::Box,
    pub(crate) spotlight_recent_detail_repo_value: gtk::Label,
    pub(crate) spotlight_recent_detail_download_value: gtk::Label,
    pub(crate) spotlight_recent_detail_updated_row: gtk::Box,
    pub(crate) spotlight_recent_detail_updated_value: gtk::Label,
    pub(crate) spotlight_recent_detail_homepage_row: gtk::Box,
    pub(crate) spotlight_recent_detail_homepage_link: gtk::LinkButton,
    pub(crate) spotlight_recent_detail_maintainer_row: gtk::Box,
    pub(crate) spotlight_recent_detail_maintainer_value: gtk::Label,
    pub(crate) spotlight_recent_detail_license_row: gtk::Box,
    pub(crate) spotlight_recent_detail_license_value: gtk::Label,
    pub(crate) spotlight_recent_detail_status: gtk::Label,
    pub(crate) spotlight_recent_detail_description: gtk::Label,
    pub(crate) spotlight_recent_detail_update_label: gtk::Label,
    pub(crate) spotlight_recent_detail_dependencies_stack: gtk::Stack,
    pub(crate) spotlight_recent_detail_dependencies_list: gtk::ListBox,
    pub(crate) spotlight_recent_detail_dependencies_placeholder: gtk::Label,
    pub(crate) spotlight_recent_action_button: gtk::Button,
    pub(crate) spotlight_section_box: gtk::Box,
    pub(crate) category_browsers_button: gtk::ToggleButton,
    pub(crate) category_chat_button: gtk::ToggleButton,
    pub(crate) category_email_button: gtk::ToggleButton,
    pub(crate) category_games_button: gtk::ToggleButton,
    pub(crate) category_graphics_button: gtk::ToggleButton,
    pub(crate) category_music_button: gtk::ToggleButton,
    pub(crate) category_productivity_button: gtk::ToggleButton,
    pub(crate) category_utilities_button: gtk::ToggleButton,
    pub(crate) category_video_button: gtk::ToggleButton,
    pub(crate) spotlight_refresh_button: gtk::Button,
}

pub(crate) fn build_page() -> (gtk::Box, DiscoverWidgets) {
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
