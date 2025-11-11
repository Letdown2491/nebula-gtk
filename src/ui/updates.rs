use gtk::pango;
use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;

pub(crate) struct UpdatesWidgets {
    pub(crate) summary_row: gtk::Box,
    pub(crate) status_label: gtk::Label,
    pub(crate) status_revealer: gtk::Revealer,
    pub(crate) list: gtk::ListBox,
    pub(crate) scroller: gtk::ScrolledWindow,
    pub(crate) content_row: gtk::Box,
    pub(crate) placeholder: gtk::Box,
    pub(crate) placeholder_label: gtk::Label,
    pub(crate) check_button: gtk::Button,
    pub(crate) refresh_button: gtk::Button,
    pub(crate) update_all_button: gtk::Button,
    pub(crate) spinner: gtk::Spinner,
    pub(crate) summary_label: gtk::Label,
    pub(crate) footer_label: gtk::Label,
    pub(crate) detail_frame: gtk::Frame,
    pub(crate) detail_stack: gtk::Stack,
    pub(crate) detail_name: gtk::Label,
    pub(crate) detail_close_button: gtk::Button,
    pub(crate) detail_version_value: gtk::Label,
    pub(crate) detail_download_value: gtk::Label,
    pub(crate) detail_homepage_row: gtk::Box,
    pub(crate) detail_homepage_link: gtk::Label,
    pub(crate) detail_maintainer_row: gtk::Box,
    pub(crate) detail_maintainer_value: gtk::Label,
    pub(crate) detail_license_row: gtk::Box,
    pub(crate) detail_license_value: gtk::Label,
    pub(crate) detail_description: gtk::Label,
    pub(crate) detail_update_label: gtk::Label,
    pub(crate) detail_required_by_stack: gtk::Stack,
    pub(crate) detail_required_by_list: gtk::ListBox,
    pub(crate) detail_required_by_placeholder: gtk::Label,
    pub(crate) detail_update_button: gtk::Button,
}

pub(crate) fn build_page() -> (gtk::Box, UpdatesWidgets) {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(16)
        .margin_end(16)
        .margin_top(0)
        .margin_bottom(16)
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

    // Status footer with revealer (like Tools page)
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

    let detail_homepage_link = gtk::Label::builder()
        .use_markup(true)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .halign(gtk::Align::Start)
        .visible(false)
        .build();
    detail_homepage_link.set_hexpand(true);
    detail_homepage_link.set_xalign(0.0);
    detail_homepage_link.set_selectable(false);
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
        .justify(gtk::Justification::Left)
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
    container.append(&placeholder);
    let footer_label = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    footer_label.add_css_class("dim-label");
    footer_label.set_text("Last checked — never.");

    container.append(&content_row);
    container.append(&status_revealer);
    container.append(&footer_label);

    let widgets = UpdatesWidgets {
        summary_row: summary_row.clone(),
        status_label,
        status_revealer,
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
