use std::collections::HashSet;
use std::rc::Rc;
use std::thread;

use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use gtk::glib;

use crate::details::InstalledDetail;
use crate::helpers::{
    clear_listbox, format_relative_time, glib_datetime_to_chrono, package_matches_filter,
    query_installed_detail, sanitize_contact_field,
};
use crate::state::controller::AppController;
use crate::state::types::{AppMessage, InstalledFilter, RemoveOrigin};
use crate::types::PackageInfo;
use crate::xbps::{format_download_size, run_xbps_list_installed};

impl AppController {
    pub(crate) fn refresh_installed_packages(self: &Rc<Self>) {
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

    pub(crate) fn on_installed_search_changed(self: &Rc<Self>, query: String) {
        {
            let mut state = self.state.borrow_mut();
            state.installed_filter = query;
        }
        self.rebuild_installed_list();
    }

    pub(crate) fn on_installed_filter_changed(self: &Rc<Self>, selected: u32) {
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

    pub(crate) fn on_installed_remove_selected(self: &Rc<Self>) {
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

    pub(crate) fn on_installed_row_selected(self: &Rc<Self>, row: Option<gtk::ListBoxRow>) {
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

    pub(crate) fn on_installed_selection_toggled(self: &Rc<Self>, package: String, selected: bool) {
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

    pub(crate) fn on_installed_detail_remove(self: &Rc<Self>) {
        let package = {
            let state = self.state.borrow();
            state.installed_detail_package.clone()
        };

        if let Some(pkg) = package {
            self.start_remove(pkg, RemoveOrigin::Installed);
        }
    }

    pub(crate) fn on_installed_detail_update(self: &Rc<Self>) {
        let package = {
            let state = self.state.borrow();
            state.installed_detail_package.clone()
        };

        if let Some(pkg) = package {
            self.start_update(pkg, false);
        }
    }

    pub(crate) fn on_installed_detail_close(self: &Rc<Self>) {
        self.widgets.installed.list.unselect_all();
        self.clear_installed_detail_history();
        self.clear_installed_detail();
        self.update_installed_detail_back_button();
        self.update_installed_summary();
    }

    pub(crate) fn request_installed_detail(&self, package: &str) {
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

    pub(crate) fn finish_installed_refresh(
        self: &Rc<Self>,
        result: Result<Vec<PackageInfo>, String>,
    ) {
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

    pub(crate) fn finish_installed_detail(
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

    pub(crate) fn update_installed_summary(&self) {
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
                "Last refreshed —".to_string()
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

    pub(crate) fn update_installed_selection_ui(&self) {
        self.update_installed_summary();
    }

    pub(crate) fn update_installed_details(self: &Rc<Self>) {
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
            self.widgets
                .installed
                .detail_stack
                .set_visible_child_name("detail");
            self.widgets.installed.detail_close_button.set_visible(true);
            self.widgets
                .installed
                .detail_close_button
                .set_sensitive(true);
            self.widgets.installed.detail_name.set_text(&pkg.name);

            let (detail, loading, error, remove_in_progress, update_in_progress) = {
                let state = self.state.borrow();
                (
                    state.installed_detail_cache.get(&pkg.name).cloned(),
                    state.installed_detail_loading.contains(&pkg.name),
                    state.installed_detail_errors.get(&pkg.name).cloned(),
                    state.remove_in_progress,
                    state.update_in_progress,
                )
            };

            let mut description_body = String::new();
            description_body.push_str(&pkg.description);
            if let Some(desc) = detail
                .as_ref()
                .and_then(|d| d.long_description.as_ref())
                .filter(|d| !d.trim().is_empty())
            {
                if !description_body.is_empty() {
                    description_body.push_str("\n\n");
                }
                description_body.push_str(desc.trim());
            }
            self.widgets
                .installed
                .detail_description
                .set_text(&description_body);

            if pkg.version.is_empty() {
                self.widgets.installed.detail_version_value.set_text("—");
            } else {
                self.widgets
                    .installed
                    .detail_version_value
                    .set_text(pkg.version.as_str());
            }

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

            let has_update = updates.contains(&pkg.name);
            self.widgets
                .installed
                .detail_update_label
                .set_visible(has_update);
            if has_update {
                self.widgets
                    .installed
                    .detail_update_label
                    .set_text("Update available in Updates tab.");
            } else {
                self.widgets.installed.detail_update_label.set_text("");
            }

            self.update_installed_required_by_ui(detail.as_ref(), loading, error.as_ref());
            self.set_installed_row_buttons_visible(false);

            self.widgets
                .installed
                .detail_remove_button
                .set_sensitive(!remove_in_progress);
            self.widgets
                .installed
                .detail_update_button
                .set_sensitive(!update_in_progress);
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

    pub(crate) fn on_installed_detail_back(self: &Rc<Self>) {
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
                self.widgets.installed.list.select_row(Some(&row));
            }
        }

        self.switch_to_page("installed");

        true
    }

    pub(crate) fn rebuild_installed_list(self: &Rc<Self>) {
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
}
