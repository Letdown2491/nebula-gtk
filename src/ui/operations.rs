use gtk::pango;
use gtk4 as gtk;
use libadwaita as adw;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::state::controller::AppController;
use crate::state::types::{OperationStatus, OperationType, PackageOperation};

pub(crate) fn show_operations_dialog(controller: &Rc<AppController>, parent: &adw::ApplicationWindow) {
    let window = adw::Window::builder()
        .title("Recent Operations")
        .transient_for(parent)
        .modal(true)
        .default_width(720)
        .default_height(540)
        .build();

    let toolbar_view = adw::ToolbarView::new();

    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let operations = controller.get_all_operations();

    if operations.is_empty() {
        let status_page = adw::StatusPage::builder()
            .icon_name("emblem-ok-symbolic")
            .title("No Recent Operations")
            .description("Package operations will appear here once you install, remove, or update packages.")
            .vexpand(true)
            .build();
        content.append(&status_page);
    } else {
        let list_box = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .build();
        list_box.add_css_class("boxed-list");

        for operation in operations {
            let row = build_operation_row(&operation);
            list_box.append(&row);
        }

        scrolled.set_child(Some(&list_box));
        content.append(&scrolled);

        // Footer with Clear History button
        let footer = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::Center)
            .margin_top(12)
            .margin_bottom(12)
            .build();

        let clear_button = gtk::Button::builder()
            .label("Clear History")
            .build();
        clear_button.add_css_class("destructive-action");

        let controller_weak = Rc::downgrade(controller);
        let window_weak = window.downgrade();
        clear_button.connect_clicked(move |_| {
            if let Some(controller) = controller_weak.upgrade() {
                controller.clear_operation_history();
                if let Some(window) = window_weak.upgrade() {
                    window.close();
                }
            }
        });

        footer.append(&clear_button);
        content.append(&footer);
    }

    toolbar_view.set_content(Some(&content));
    window.set_content(Some(&toolbar_view));
    window.present();
}

fn build_operation_row(operation: &PackageOperation) -> adw::ExpanderRow {
    let title = format_operation_title(operation);
    let subtitle = format_operation_subtitle(operation);

    let row = adw::ExpanderRow::builder()
        .title(&title)
        .subtitle(&subtitle)
        .build();

    // Status icon
    let (icon_name, css_class) = match operation.status {
        OperationStatus::InProgress => ("emblem-synchronizing-symbolic", "accent"),
        OperationStatus::Success => ("emblem-ok-symbolic", "success"),
        OperationStatus::Warning => ("dialog-warning-symbolic", "warning"),
        OperationStatus::Failed => ("dialog-error-symbolic", "error"),
    };

    let status_icon = gtk::Image::builder()
        .icon_name(icon_name)
        .valign(gtk::Align::Center)
        .build();
    status_icon.add_css_class(css_class);
    row.add_prefix(&status_icon);

    // Add details in expanded section
    if operation.is_complete() {
        let details_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();

        // Command
        let command_escaped = glib::markup_escape_text(&operation.command);
        let command_label = gtk::Label::builder()
            .label(&format!("<b>Command:</b> {}", command_escaped))
            .use_markup(true)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .selectable(true)
            .build();
        details_box.append(&command_label);

        // Duration
        if let Some(duration) = operation.duration_seconds() {
            let duration_label = gtk::Label::builder()
                .label(&format!("<b>Duration:</b> {} seconds", duration))
                .use_markup(true)
                .xalign(0.0)
                .build();
            details_box.append(&duration_label);
        }

        // Exit code
        if let Some(code) = operation.exit_code {
            let exit_label = gtk::Label::builder()
                .label(&format!("<b>Exit code:</b> {}", code))
                .use_markup(true)
                .xalign(0.0)
                .build();
            details_box.append(&exit_label);
        }

        // Error message
        if let Some(ref error_msg) = operation.error_message {
            let error_frame = gtk::Frame::builder()
                .margin_top(8)
                .build();
            error_frame.add_css_class("error");

            let error_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(4)
                .margin_start(12)
                .margin_end(12)
                .margin_top(8)
                .margin_bottom(8)
                .build();

            let error_title = gtk::Label::builder()
                .label("<b>Error:</b>")
                .use_markup(true)
                .xalign(0.0)
                .build();

            let error_escaped = glib::markup_escape_text(error_msg);
            let error_text = gtk::Label::builder()
                .label(error_escaped.as_str())
                .xalign(0.0)
                .wrap(true)
                .wrap_mode(pango::WrapMode::WordChar)
                .selectable(true)
                .build();

            error_box.append(&error_title);
            error_box.append(&error_text);
            error_frame.set_child(Some(&error_box));
            details_box.append(&error_frame);
        }

        // Stdout (if not empty and no error, or if user wants details)
        if !operation.stdout.trim().is_empty() {
            let stdout_expander = gtk::Expander::builder()
                .label("Standard Output")
                .margin_top(8)
                .build();

            let stdout_scrolled = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Automatic)
                .vscrollbar_policy(gtk::PolicyType::Automatic)
                .max_content_height(400)
                .build();

            let stdout_text = gtk::TextView::builder()
                .editable(false)
                .cursor_visible(false)
                .monospace(true)
                .wrap_mode(gtk::WrapMode::Word)
                .margin_start(8)
                .margin_end(8)
                .margin_top(8)
                .margin_bottom(8)
                .build();
            stdout_text.buffer().set_text(&operation.stdout);
            stdout_scrolled.set_child(Some(&stdout_text));
            stdout_expander.set_child(Some(&stdout_scrolled));
            details_box.append(&stdout_expander);
        }

        // Stderr (if not empty)
        if !operation.stderr.trim().is_empty() && operation.error_message.is_none() {
            let stderr_expander = gtk::Expander::builder()
                .label("Standard Error")
                .margin_top(8)
                .build();

            let stderr_scrolled = gtk::ScrolledWindow::builder()
                .hscrollbar_policy(gtk::PolicyType::Automatic)
                .vscrollbar_policy(gtk::PolicyType::Automatic)
                .max_content_height(400)
                .build();

            let stderr_text = gtk::TextView::builder()
                .editable(false)
                .cursor_visible(false)
                .monospace(true)
                .wrap_mode(gtk::WrapMode::Word)
                .margin_start(8)
                .margin_end(8)
                .margin_top(8)
                .margin_bottom(8)
                .build();
            stderr_text.buffer().set_text(&operation.stderr);
            stderr_scrolled.set_child(Some(&stderr_text));
            stderr_expander.set_child(Some(&stderr_text));
            details_box.append(&stderr_expander);
        }

        row.add_row(&details_box);
    }

    row
}

fn format_operation_title(operation: &PackageOperation) -> String {
    let op_type = match &operation.operation_type {
        OperationType::Install => "Installed",
        OperationType::Remove => "Removed",
        OperationType::Update { .. } => "Updated",
    };

    format!("{} {}", op_type, operation.package_name)
}

fn format_operation_subtitle(operation: &PackageOperation) -> String {
    let mut parts = Vec::new();

    // Add version info for updates
    if let OperationType::Update { from_version, to_version } = &operation.operation_type {
        parts.push(format!("{} → {}", from_version, to_version));
    }

    // Add timestamp
    let local_time = operation.started_at.with_timezone(&chrono::Local);
    let time_str = local_time.format("%b %d, %Y at %I:%M %p").to_string();
    parts.push(time_str);

    // Add status label for non-success states
    match operation.status {
        OperationStatus::InProgress => parts.push("In progress…".to_string()),
        OperationStatus::Warning => parts.push("Completed with warnings".to_string()),
        OperationStatus::Failed => parts.push("Failed".to_string()),
        OperationStatus::Success => {}
    }

    parts.join(" • ")
}
