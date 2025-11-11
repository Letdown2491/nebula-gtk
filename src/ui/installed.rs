use gtk::gio;
use gtk::glib;
use gtk::pango;
use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use gtk::{BaselinePosition, Justification};

pub(crate) struct InstalledWidgets {
    pub(crate) refresh_button: gtk::Button,
    pub(crate) search_entry: gtk::SearchEntry,
    pub(crate) status_label: gtk::Label,
    pub(crate) spinner: gtk::Spinner,
    pub(crate) filter_dropdown: gtk::DropDown,
    pub(crate) remove_selected_button: gtk::Button,
    pub(crate) list_store: gio::ListStore,
    pub(crate) list_selection: gtk::SingleSelection,
    pub(crate) list_view: gtk::ListView,
    pub(crate) list_factory: gtk::SignalListItemFactory,
    pub(crate) installed_results_stack: gtk::Stack,
    pub(crate) no_results_page: adw::StatusPage,
    pub(crate) detail_stack: gtk::Stack,
    pub(crate) detail_frame: gtk::Frame,
    pub(crate) detail_remove_button: gtk::Button,
    pub(crate) detail_update_button: gtk::Button,
    pub(crate) detail_pin_button: gtk::Button,
    pub(crate) detail_back_button: gtk::Button,
    pub(crate) detail_close_button: gtk::Button,
    pub(crate) detail_name: gtk::Label,
    pub(crate) detail_version_value: gtk::Label,
    pub(crate) detail_description: gtk::Label,
    pub(crate) detail_download_value: gtk::Label,
    pub(crate) detail_homepage_row: gtk::Box,
    pub(crate) detail_homepage_link: gtk::Label,
    pub(crate) detail_maintainer_row: gtk::Box,
    pub(crate) detail_maintainer_value: gtk::Label,
    pub(crate) detail_license_row: gtk::Box,
    pub(crate) detail_license_value: gtk::Label,
    pub(crate) detail_required_by_stack: gtk::Stack,
    pub(crate) detail_required_by_list: gtk::ListBox,
    pub(crate) detail_required_by_placeholder: gtk::Label,
    pub(crate) detail_update_label: gtk::Label,
    pub(crate) footer_label: gtk::Label,
}

pub(crate) fn build_page() -> (gtk::Box, InstalledWidgets) {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(16)
        .margin_end(16)
        .margin_top(0)
        .margin_bottom(16)
        .build();
    container.set_vexpand(true);

    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search installed packages")
        .hexpand(true)
        .build();
    search_entry.set_valign(gtk::Align::Center);

    let search_bar = gtk::SearchBar::new();
    search_bar.set_hexpand(true);
    search_bar.set_search_mode(true);
    search_bar.set_show_close_button(false);
    search_bar.set_key_capture_widget(Some(&container));
    search_bar.connect_entry(&search_entry);
    search_bar.set_child(Some(&search_entry));

    let filter_model = gtk::StringList::new(&["All packages", "Updates available"]);
    let filter_dropdown = gtk::DropDown::builder()
        .model(&filter_model)
        .selected(0)
        .build();
    filter_dropdown.set_hexpand(false);
    filter_dropdown.add_css_class("nebula-compact-dropdown");
    filter_dropdown.set_valign(gtk::Align::Center);

    let controls_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .hexpand(true)
        .build();
    controls_row.append(&search_bar);
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
    status_row.set_baseline_position(BaselinePosition::Center);
    status_row.append(&refresh_button);
    status_row.append(&status_label);
    status_row.append(&spinner);
    status_row.append(&remove_selected_button);

    let list_store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let list_selection = gtk::SingleSelection::new(Some(list_store.clone()));
    list_selection.set_autoselect(false);
    list_selection.set_can_unselect(true);

    let list_factory = gtk::SignalListItemFactory::new();

    let list_view = gtk::ListView::new(Some(list_selection.clone()), Some(list_factory.clone()));
    list_view.add_css_class("boxed-list");
    list_view.set_vexpand(true);
    list_view.set_hexpand(true);
    list_view.set_single_click_activate(false);

    let scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(320)
        .build();
    scroller.set_child(Some(&list_view));

    // Empty state for no results
    let no_results_page = adw::StatusPage::builder()
        .icon_name("system-search-symbolic")
        .title("No Packages Found")
        .description("No installed packages match your search.")
        .vexpand(true)
        .hexpand(true)
        .build();

    // Stack to switch between list and no-results
    let installed_results_stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .hexpand(true)
        .vexpand(true)
        .build();
    installed_results_stack.add_named(&scroller, Some("list"));
    installed_results_stack.add_named(&no_results_page, Some("no-results"));
    installed_results_stack.set_visible_child_name("list");

    let detail_name = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .build();
    detail_name.add_css_class("title-2");
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

    let detail_description = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .wrap_mode(pango::WrapMode::Word)
        .hexpand(true)
        .justify(Justification::Left)
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

    let detail_pin_button = gtk::Button::builder()
        .label("Hold")
        .width_request(120)
        .width_request(120)
        .build();
    detail_pin_button.set_halign(gtk::Align::Start);
    detail_pin_button.set_visible(false);
    detail_pin_button.set_valign(gtk::Align::Center);
    detail_pin_button.set_tooltip_text(Some(
        "Prevent this package from being updated during system upgrades.",
    ));

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
    detail_actions_row.append(&detail_update_button);
    detail_actions_row.append(&detail_pin_button);
    detail_actions_row.append(&detail_remove_button);
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
    detail_placeholder.set_justify(Justification::Center);

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
    content_row.append(&installed_results_stack);
    content_row.append(&detail_frame);

    container.append(&controls_row);
    container.append(&status_row);
    container.append(&content_row);
    let footer_label = gtk::Label::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .wrap(true)
        .wrap_mode(pango::WrapMode::WordChar)
        .margin_top(6)
        .margin_bottom(6)
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
        list_store,
        list_selection,
        list_view,
        list_factory,
        installed_results_stack,
        no_results_page,
        detail_stack,
        detail_frame,
        detail_remove_button,
        detail_update_button,
        detail_pin_button,
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
