use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use gtk::gio;
use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use gtk::glib;

use crate::categories::icon_resource_for_package;
use crate::details::InstalledDetail;
use crate::helpers::{
    clear_listbox, format_relative_time, glib_datetime_to_chrono, query_installed_detail,
    sanitize_contact_field, select_row_if_attached, set_link_label,
};
use crate::mirrors::install_repository_args;
use crate::state::controller::AppController;
use crate::state::types::{AppMessage, AppState, UpdateStatus};
use crate::types::{CommandResult, PackageInfo};
use crate::xbps::{format_download_size, run_xbps_check_updates, split_package_identifier};

impl AppController {
    pub(crate) fn set_check_buttons_sensitive(&self, enabled: bool) {
        self.widgets.updates.check_button.set_sensitive(enabled);
        self.widgets.updates.refresh_button.set_sensitive(enabled);
        self.widgets
            .updates
            .update_all_button
            .set_sensitive(enabled);
    }

    pub(crate) fn set_status_text(&self, text: &str) {
        let label = &self.widgets.updates.status_label;
        label.set_text(text);
        label.set_visible(!text.is_empty());
    }

    pub(crate) fn set_summary_text(&self, text: &str) {
        self.widgets.updates.summary_label.set_text(text);
        let should_show = !text.is_empty() || self.widgets.updates.spinner.is_visible();
        self.widgets
            .updates
            .summary_label
            .set_visible(!text.is_empty());
        self.widgets.updates.summary_row.set_visible(should_show);
    }

    pub(crate) fn update_summary_text(&self) {
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

    pub(crate) fn update_footer_text(&self) {
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
                    "Last checked just now.".to_string()
                }
            } else {
                "Last checked — never.".to_string()
            }
        };

        self.widgets.updates.footer_label.set_text(&text);
    }

    pub(crate) fn update_updates_badge(&self) {
        let count = self.state.borrow().available_updates.len();
        self.widgets.updates_page.set_badge_number(count as u32);
    }

    pub(crate) fn maybe_notify_new_updates(&self, count: usize) {
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

    pub(crate) fn withdraw_updates_notification(&self) {
        self.app.withdraw_notification("updates");
    }

    pub(crate) fn rebuild_updates_list(self: &Rc<Self>) {
        let list = &self.widgets.updates.list;
        clear_listbox(list);

        let (updates, selected, busy, detail_open, statuses) = {
            let state = self.state.borrow();
            (
                state.available_updates.clone(),
                state.selected_updates.clone(),
                state.update_in_progress || state.updates_loading,
                state.updates_detail_package.is_some(),
                state.update_statuses.clone(),
            )
        };
        self.update_buttons.borrow_mut().clear();

        for pkg in &updates {
            let is_selected = selected.contains(&pkg.name);
            let status = statuses.get(&pkg.name).copied();
            let row = self.build_update_row(pkg, busy, detail_open, is_selected, status);
            list.append(&row);
        }

        let detail_target = {
            let state = self.state.borrow();
            state.updates_detail_package.clone()
        };

        if let Some(target) = detail_target {
            if let Some(idx) = updates.iter().position(|pkg| pkg.name == target) {
                if let Some(row) = list.row_at_index(idx as i32) {
                    select_row_if_attached(list, &row);
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
        status: Option<UpdateStatus>,
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
        check_button.connect_toggled(glib::clone!(
            #[strong(rename_to = controller)]
            self,
            move |btn| {
                controller.on_update_selection_changed(package_name.clone(), btn.is_active());
            }
        ));

        let icon = gtk::Image::from_resource(icon_resource_for_package(&pkg.name));
        icon.set_pixel_size(32);
        icon.set_margin_start(8);
        icon.set_margin_end(6);
        icon.set_valign(gtk::Align::Center);

        let prefix_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .valign(gtk::Align::Center)
            .build();
        prefix_box.append(&check_button);
        prefix_box.append(&icon);
        row.add_prefix(&prefix_box);

        if !version_label_text.is_empty() {
            let version_label = gtk::Label::new(Some(version_label_text.as_str()));
            version_label.add_css_class("dim-label");
            version_label.set_halign(gtk::Align::End);
            version_label.set_valign(gtk::Align::Center);
            version_label.set_margin_end(12);
            row.add_suffix(&version_label);
        }

        let button_label = status
            .map(|state| {
                if matches!(state, UpdateStatus::Failed) {
                    "Update"
                } else {
                    state.label()
                }
            })
            .unwrap_or("Update");
        let update_button = gtk::Button::builder().label(button_label).build();
        update_button.add_css_class("suggested-action");
        let can_interact = match status {
            Some(UpdateStatus::Failed) | None => !disabled,
            Some(_) => false,
        };
        update_button.set_sensitive(can_interact);
        update_button.set_valign(gtk::Align::Center);
        update_button.set_margin_start(12);
        update_button.set_visible(!detail_open);

        let package_name = pkg.name.clone();
        update_button.connect_clicked(glib::clone!(
            #[strong(rename_to = controller)]
            self,
            move |_| {
                controller.start_update(package_name.clone(), false);
            }
        ));

        row.add_suffix(&update_button);
        self.update_buttons
            .borrow_mut()
            .insert(pkg.name.clone(), update_button.clone());

        row
    }

    pub(crate) fn on_update_selection_changed(self: &Rc<Self>, package: String, selected: bool) {
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

    pub(crate) fn update_update_controls(self: &Rc<Self>) {
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

        if updating {
            self.widgets.updates.update_all_button.set_label("Updating");
            self.widgets.updates.update_all_button.set_sensitive(false);
            self.update_updates_detail();
            return;
        }

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

    pub(crate) fn on_update_row_activated(self: &Rc<Self>, row: &gtk::ListBoxRow) {
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

    pub(crate) fn on_update_row_selected(self: &Rc<Self>, row: Option<gtk::ListBoxRow>) {
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

    pub(crate) fn on_updates_detail_update(self: &Rc<Self>) {
        let package = {
            let state = self.state.borrow();
            state.updates_detail_package.clone()
        };

        if let Some(pkg) = package {
            self.start_update(pkg, false);
        }
    }

    pub(crate) fn finish_updates_detail(
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

    pub(crate) fn request_updates_detail(&self, package: &str) {
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

    pub(crate) fn clear_updates_detail(&self) {
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

    pub(crate) fn update_updates_detail(self: &Rc<Self>) {
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
                    set_link_label(&widgets.detail_homepage_link, Some(home));
                } else {
                    widgets.detail_homepage_row.set_visible(false);
                    set_link_label(&widgets.detail_homepage_link, None);
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
                set_link_label(&widgets.detail_homepage_link, None);
                widgets.detail_maintainer_row.set_visible(false);
                widgets.detail_maintainer_value.set_visible(false);
                widgets.detail_license_row.set_visible(false);
                widgets.detail_license_value.set_visible(false);
            }

            widgets.detail_update_label.set_visible(false);
            widgets.detail_update_button.set_sensitive(!loading);
            widgets.detail_update_button.set_visible(pkg_info.is_some());

            let status = {
                let state = self.state.borrow();
                state.update_statuses.get(&pkg_name).copied()
            };
            self.update_detail_button_label(&pkg_name, status);

            self.update_updates_required_by_ui(detail.as_ref(), loading, error.as_ref());
        } else {
            self.clear_updates_detail();
        }
    }

    pub(crate) fn set_all_update_row_buttons_visible(&self, visible: bool) {
        for button in self.update_buttons.borrow().values() {
            button.set_visible(visible);
        }
    }

    pub(crate) fn on_update_log_line(self: &Rc<Self>, line: String) {
        let cleaned = line.trim_end_matches('\r').to_string();
        {
            let mut state = self.state.borrow_mut();
            state.update_log.push(cleaned.clone());
        }
        self.append_update_log_buffer_line(&cleaned);
        self.update_status_from_log_line(&cleaned);
    }

    fn update_status_from_log_line(&self, line: &str) {
        let candidates = {
            let state = self.state.borrow();
            state
                .update_statuses
                .keys()
                .map(|name| (name.clone(), name.to_ascii_lowercase()))
                .collect::<Vec<_>>()
        };
        if candidates.is_empty() {
            return;
        }

        let lower_line = line.to_ascii_lowercase();
        let mut matched = self.detect_packages_in_line(line, &lower_line, &candidates);

        if matched.is_empty() {
            if lower_line.contains("transaction aborted")
                || lower_line.contains("failed to download")
                || lower_line.contains("failed to install")
                || lower_line.contains("failed to update")
            {
                let packages: Vec<String> =
                    candidates.iter().map(|(name, _)| name.clone()).collect();
                self.set_packages_status(&packages, UpdateStatus::Failed);
            }
            return;
        }

        let status = Self::status_from_keywords(&lower_line);
        for package in matched.drain(..) {
            if let Some(stage) = status {
                self.set_packages_status(&[package.clone()], stage);
            } else {
                self.maybe_mark_package_preparing(&package);
            }
        }
    }

    fn detect_packages_in_line(
        &self,
        line: &str,
        lower_line: &str,
        candidates: &[(String, String)],
    ) -> Vec<String> {
        let mut matches = Vec::new();
        for (original, lower_name) in candidates {
            if lower_line.contains(&format!(" {}-", lower_name))
                || lower_line.contains(&format!(" {} ", lower_name))
                || lower_line.contains(&format!(" {}:", lower_name))
                || lower_line.contains(&format!(" {}_", lower_name))
                || lower_line.starts_with(&format!("{}-", lower_name))
                || lower_line.starts_with(&format!("{}:", lower_name))
                || lower_line.contains(&format!("`{}`", lower_name))
            {
                matches.push(original.clone());
            }
        }

        if matches.len() == candidates.len() {
            return matches;
        }

        for token in line.split_whitespace() {
            let trimmed = token.trim_matches(|ch: char| {
                matches!(
                    ch,
                    '(' | ')' | ':' | ',' | ';' | '.' | '`' | '\'' | '"' | '[' | ']' | '{' | '}'
                )
            });
            if trimmed.is_empty() {
                continue;
            }

            let lower_trimmed = trimmed.to_ascii_lowercase();
            for (original, lower_name) in candidates {
                if lower_trimmed == *lower_name && !matches.contains(original) {
                    matches.push(original.clone());
                }
            }

            let (identifier_name, _) = split_package_identifier(trimmed);
            if !identifier_name.is_empty()
                && candidates
                    .iter()
                    .any(|(candidate, _)| candidate == &identifier_name)
                && !matches.contains(&identifier_name)
            {
                matches.push(identifier_name);
            }
        }

        matches
    }

    fn status_from_keywords(lower_line: &str) -> Option<UpdateStatus> {
        if lower_line.contains("failed")
            || lower_line.contains("error:")
            || lower_line.contains("transaction aborted")
        {
            Some(UpdateStatus::Failed)
        } else if lower_line.contains("downloading") || lower_line.contains("fetching") {
            Some(UpdateStatus::Downloading)
        } else if lower_line.contains("installing")
            || lower_line.contains("updating")
            || lower_line.contains("unpacking")
        {
            Some(UpdateStatus::Installing)
        } else if lower_line.contains("verifying") || lower_line.contains("checking integrity") {
            Some(UpdateStatus::Verifying)
        } else if lower_line.contains("installed successfully")
            || lower_line.contains("update completed")
            || lower_line.contains("updated successfully")
            || lower_line.contains("transaction completed")
            || lower_line.contains("upgraded successfully")
        {
            Some(UpdateStatus::Completed)
        } else if lower_line.contains("preparing") || lower_line.contains("transaction started") {
            Some(UpdateStatus::Preparing)
        } else {
            None
        }
    }

    fn maybe_mark_package_preparing(&self, package: &str) {
        let should_update = {
            let state = self.state.borrow();
            matches!(
                state.update_statuses.get(package),
                Some(UpdateStatus::Queued)
            )
        };
        if should_update {
            self.set_packages_status(&[package.to_string()], UpdateStatus::Preparing);
        }
    }

    fn set_packages_status(&self, packages: &[String], status: UpdateStatus) {
        let mut changed = Vec::new();
        {
            let mut state = self.state.borrow_mut();
            for name in packages {
                let replace = match state.update_statuses.get(name) {
                    Some(current) if !status.should_replace(*current) => false,
                    _ => true,
                };
                if replace {
                    state.update_statuses.insert(name.clone(), status);
                    changed.push(name.clone());
                }
            }
        }
        if !changed.is_empty() {
            self.update_package_status_buttons(&changed);
        }
    }

    fn clear_package_status(&self, packages: &[String]) {
        let mut changed = Vec::new();
        {
            let mut state = self.state.borrow_mut();
            for name in packages {
                if state.update_statuses.remove(name).is_some() {
                    changed.push(name.clone());
                }
            }
        }
        if !changed.is_empty() {
            self.update_package_status_buttons(&changed);
        }
    }

    fn update_package_status_buttons(&self, packages: &[String]) {
        let (statuses, updating) = {
            let state = self.state.borrow();
            (
                packages
                    .iter()
                    .map(|name| (name.clone(), state.update_statuses.get(name).copied()))
                    .collect::<Vec<_>>(),
                state.update_in_progress,
            )
        };

        let buttons = self.update_buttons.borrow();
        for (name, status) in statuses {
            if let Some(button) = buttons.get(&name) {
                let label = match status {
                    Some(UpdateStatus::Failed) | None => "Update",
                    Some(other) => other.label(),
                };
                button.set_label(label);
                let sensitive = match status {
                    Some(UpdateStatus::Failed) => true,
                    Some(_) => false,
                    None => !updating,
                };
                button.set_sensitive(sensitive);
            }
            self.update_detail_button_label(&name, status);
        }
    }

    fn update_detail_button_label(&self, package: &str, status: Option<UpdateStatus>) {
        let (matches, updating) = {
            let state = self.state.borrow();
            (
                state
                    .updates_detail_package
                    .as_ref()
                    .map(|name| name == package)
                    .unwrap_or(false),
                state.update_in_progress,
            )
        };

        if !matches {
            return;
        }

        let label = match status {
            Some(UpdateStatus::Failed) | None => "Update",
            Some(other) => other.label(),
        };
        let button = &self.widgets.updates.detail_update_button;
        button.set_label(label);
        let sensitive = match status {
            Some(UpdateStatus::Failed) => true,
            Some(_) => false,
            None => !updating,
        };
        button.set_sensitive(sensitive);
    }

    pub(crate) fn on_updates_detail_close(self: &Rc<Self>) {
        self.widgets.updates.list.unselect_all();
        self.clear_updates_detail();
    }

    pub(crate) fn update_updates_required_by_ui(
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
                widgets.detail_required_by_list.set_visible(false);
                widgets
                    .detail_required_by_stack
                    .set_visible_child_name("placeholder");
                return;
            }

            if detail.required_by.is_empty() {
                widgets
                    .detail_required_by_placeholder
                    .set_text("Not required by any installed package.");
                widgets.detail_required_by_list.set_visible(false);
                widgets
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
        widgets.detail_required_by_list.set_visible(false);
        widgets
            .detail_required_by_stack
            .set_visible_child_name("placeholder");
    }

    pub(crate) fn sync_updates_detail_state(&self) {
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

    pub(crate) fn refresh_updates(self: &Rc<Self>, silent: bool) {
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

    pub(crate) fn finish_updates_refresh(
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
                let available_names_snapshot = state.available_update_names.clone();
                state
                    .update_statuses
                    .retain(|name, _| available_names_snapshot.contains(name));
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

    fn refresh_available_update_names(state: &mut AppState) {
        state.available_update_names.clear();
        state
            .available_update_names
            .extend(state.available_updates.iter().map(|pkg| pkg.name.clone()));
    }

    pub(crate) fn update_all_packages(self: &Rc<Self>) {
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

    pub(crate) fn start_update(self: &Rc<Self>, package: String, from_all: bool) {
        self.execute_update(package, from_all);
    }

    pub(crate) fn start_update_multiple(self: &Rc<Self>, packages: Vec<String>) {
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

        {
            let mut state = self.state.borrow_mut();
            state.update_in_progress = true;
            state.update_log.clear();
        }
        self.refresh_update_log_buffer();

        if affected_packages.is_empty() {
            self.set_status_text("");
            self.set_summary_text("");
            self.set_footer_message(None);
            {
                let mut state = self.state.borrow_mut();
                state.update_in_progress = false;
            }
            return;
        }

        self.set_packages_status(&affected_packages, UpdateStatus::Queued);

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
        self.update_update_controls();

        let sender = self.sender.clone();
        if from_all {
            let packages_for_thread = affected_packages.clone();
            let args = build_update_all_args();
            thread::spawn(move || {
                let result = run_update_command(args, &sender);
                let _ = sender.send(AppMessage::UpdateFinished {
                    packages: packages_for_thread,
                    result,
                    all: true,
                });
            });
        } else {
            let packages_for_thread = affected_packages.clone();
            let args = build_update_packages_args(&packages_for_thread);
            thread::spawn(move || {
                let result = run_update_command(args, &sender);
                let _ = sender.send(AppMessage::UpdateFinished {
                    packages: packages_for_thread,
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
            state.update_log.clear();
        }
        self.refresh_update_log_buffer();

        self.set_packages_status(&packages, UpdateStatus::Queued);

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
        self.update_update_controls();

        let affected = packages.clone();
        let args = build_update_packages_args(&affected);
        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = run_update_command(args, &sender);
            let _ = sender.send(AppMessage::UpdateFinished {
                packages: affected,
                result,
                all: false,
            });
        });
    }

    pub(crate) fn finish_update(
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
                    self.clear_package_status(&packages);
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
                } else {
                    self.set_packages_status(&packages, UpdateStatus::Failed);
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
                self.set_packages_status(&packages, UpdateStatus::Failed);
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
        if !has_updates {
            self.widgets
                .updates
                .placeholder_label
                .set_text("Your system is up to date!");
            self.clear_updates_detail();
        }

        self.refresh_update_log_buffer();
        self.update_updates_badge();
        self.update_footer_text();
    }
}

fn build_update_all_args() -> Vec<String> {
    let mut args = install_repository_args();
    args.push("-y".to_string());
    args.push("-Su".to_string());
    args
}

fn build_update_packages_args(packages: &[String]) -> Vec<String> {
    let mut args = install_repository_args();
    args.push("-y".to_string());
    args.push("-u".to_string());
    for pkg in packages {
        args.push(pkg.clone());
    }
    args
}

fn run_update_command(
    args: Vec<String>,
    sender: &mpsc::Sender<AppMessage>,
) -> Result<CommandResult, String> {
    let mut command = Command::new("pkexec");
    command.arg("xbps-install");
    for arg in &args {
        command.arg(arg);
    }
    command.env("NO_COLOR", "1");
    command.env("XBPS_INSTALL_VERBOSE", "2");
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let spawn_result = command.spawn();
    let mut child = match spawn_result {
        Ok(child) => child,
        Err(err) => {
            let message = format!("Failed to launch pkexec: {}", err);
            let _ = sender.send(AppMessage::UpdateLogLine {
                line: message.clone(),
            });
            return Err(message);
        }
    };

    enum StreamEvent {
        Stdout(String),
        Stderr(String),
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, rx) = mpsc::channel::<StreamEvent>();

    if let Some(stdout) = stdout {
        let tx = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(mut text) => {
                        if text.ends_with('\r') {
                            text = text.trim_end_matches('\r').to_string();
                        }
                        if text.is_empty() {
                            continue;
                        }
                        let _ = tx.send(StreamEvent::Stdout(text));
                    }
                    Err(_) => break,
                }
            }
        });
    }

    if let Some(stderr) = stderr {
        let tx = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(mut text) => {
                        if text.ends_with('\r') {
                            text = text.trim_end_matches('\r').to_string();
                        }
                        if text.is_empty() {
                            continue;
                        }
                        let _ = tx.send(StreamEvent::Stderr(text));
                    }
                    Err(_) => break,
                }
            }
        });
    }

    drop(tx);

    let mut stdout_accum = String::new();
    let mut stderr_accum = String::new();

    for event in rx {
        match event {
            StreamEvent::Stdout(line) => {
                if !stdout_accum.is_empty() {
                    stdout_accum.push('\n');
                }
                stdout_accum.push_str(&line);
                let _ = sender.send(AppMessage::UpdateLogLine { line });
            }
            StreamEvent::Stderr(line) => {
                if !stderr_accum.is_empty() {
                    stderr_accum.push('\n');
                }
                stderr_accum.push_str(&line);
                let _ = sender.send(AppMessage::UpdateLogLine { line });
            }
        }
    }

    let status = child
        .wait()
        .map_err(|err| format!("Failed to wait for pkexec: {}", err))?;

    Ok(CommandResult {
        code: status.code(),
        stdout: stdout_accum,
        stderr: stderr_accum,
    })
}
