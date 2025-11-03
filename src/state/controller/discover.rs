use std::collections::HashMap;
use std::rc::Rc;
use std::thread;

use chrono::{DateTime, Utc};
use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use gtk::glib;
use gtk::prelude::{ListBoxRowExt, WidgetExt};

use crate::categories::icon_resource_for_package;
use crate::details::DiscoverDetail;
use crate::helpers::{
    clear_listbox, detail_download_bytes, format_relative_time, populate_spotlight_list,
    sanitize_contact_field, select_row_if_attached, set_download_label, set_link_label,
    set_toggle_button_state,
};
use crate::spotlight::{
    SPOTLIGHT_REFRESH_INTERVAL_HOURS, SpotlightCache, SpotlightCategory, category_display_name,
    refresh_spotlight_cache, save_spotlight_cache_to_disk,
};
use crate::state::controller::AppController;
use crate::state::types::{AppMessage, DiscoverMode, RemoveOrigin};
use crate::types::{PackageInfo, lowercase_cache};
use crate::xbps::{format_size, run_xbps_query_search};

impl AppController {
    pub(crate) fn on_discover_primary_action(self: &Rc<Self>) {
        let pkg = match self.current_search_selection() {
            Some(pkg) => pkg,
            None => return,
        };

        if pkg.installed {
            self.on_remove_from_discover_requested();
        } else {
            self.on_install_requested();
        }
    }

    pub(crate) fn on_search_requested(self: &Rc<Self>) {
        let query = self.widgets.discover.search_entry.text().trim().to_string();
        if query.is_empty() {
            self.set_discover_status(Some(
                "Type a package name or keyword to search the repository.",
            ));
            return;
        }

        let clear_category = {
            let state = self.state.borrow();
            state.active_spotlight_category.is_some()
        };
        if clear_category {
            self.clear_spotlight_category();
        }

        {
            let mut state = self.state.borrow_mut();
            if state.search_in_progress {
                return;
            }
            state.search_in_progress = true;
            state.discover_mode = DiscoverMode::Search;
        }

        self.update_discover_layout();

        let message = format!("Searching for \"{}\"…", query);
        self.set_discover_status(Some(&message));
        self.widgets.discover.search_entry.set_editable(false);
        self.widgets.discover.search_spinner.set_visible(true);
        self.widgets.discover.search_spinner.start();
        let preserve_navigation = {
            let state = self.state.borrow();
            state.discover_detail_navigation_active || state.pending_discover_target.is_some()
        };
        self.clear_discover_details(preserve_navigation);
        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = run_xbps_query_search(&query);
            let _ = sender.send(AppMessage::SearchFinished { query, result });
        });
    }

    pub(crate) fn on_discover_search_changed(self: &Rc<Self>, text: String) {
        if !text.trim().is_empty() {
            return;
        }

        let should_reset = {
            let mut state = self.state.borrow_mut();
            if state.discover_mode != DiscoverMode::Search {
                false
            } else {
                state.discover_mode = DiscoverMode::Spotlight;
                state.search_results.clear();
                state.selected_search = None;
                state.search_in_progress = false;
                state.discover_detail_focus = None;
                true
            }
        };

        if !should_reset {
            return;
        }

        self.widgets.discover.search_spinner.stop();
        self.widgets.discover.search_spinner.set_visible(false);
        self.widgets.discover.search_entry.set_editable(true);
        self.rebuild_search_list();
        self.clear_discover_details(false);
        self.update_discover_layout();
        self.set_discover_status(Some(
            "Type a package name or keyword to search the repository.",
        ));
    }

    pub(crate) fn on_install_requested(self: &Rc<Self>) {
        let package = match self.current_search_selection() {
            Some(pkg) if !pkg.installed => pkg,
            Some(pkg) => {
                let message = format!("\"{}\" is already installed.", pkg.name);
                self.set_discover_status(Some(&message));
                return;
            }
            None => return,
        };
        if self.state.borrow().confirm_install {
            let pkg_clone = package.clone();
            let heading = format!("Install \"{}\"?", package.name);
            let body = "Nebula will install this package and any required dependencies.";
            self.confirm_action(&heading, body, "Install", move |controller| {
                controller.begin_install(pkg_clone);
            });
            return;
        }

        self.begin_install(package);
    }

    pub(crate) fn on_remove_from_discover_requested(self: &Rc<Self>) {
        let package = match self.current_search_selection() {
            Some(pkg) if pkg.installed => pkg,
            _ => return,
        };
        self.start_remove(package.name, RemoveOrigin::Discover);
    }

    pub(crate) fn on_search_row_selected(self: &Rc<Self>, row: Option<gtk::ListBoxRow>) {
        let selected_index = row.as_ref().map(|r| r.index() as usize);
        let navigation = {
            let mut state = self.state.borrow_mut();
            let nav = state.discover_detail_navigation_active;
            if !nav {
                state.discover_detail_history.clear();
            }
            state.discover_detail_navigation_active = false;
            state.pending_discover_target = None;
            state.selected_search = selected_index;
            if let Some(idx) = selected_index {
                if let Some(pkg) = state.search_results.get(idx) {
                    state.discover_detail_focus = Some(pkg.clone());
                }
            } else if !nav {
                state.discover_detail_focus = None;
            }
            nav
        };
        if !navigation {
            self.update_discover_detail_back_button();
        }
        self.update_discover_details();
        self.update_spotlight_recent_detail();
    }

    pub(crate) fn finish_search(
        self: &Rc<Self>,
        query: String,
        result: Result<Vec<PackageInfo>, String>,
    ) {
        self.widgets.discover.search_spinner.stop();
        self.widgets.discover.search_spinner.set_visible(false);
        self.widgets.discover.search_entry.set_editable(true);
        {
            let mut state = self.state.borrow_mut();
            state.search_in_progress = false;
        }

        match result {
            Ok(mut packages) => {
                {
                    let state = self.state.borrow();
                    packages.iter_mut().for_each(|pkg| {
                        pkg.installed = state.installed_set.contains(&pkg.name);
                    });
                }

                let (pending_target, navigation_active) = {
                    let mut state = self.state.borrow_mut();
                    state.search_results = packages;
                    state.selected_search = None;
                    state.discover_detail_focus = None;
                    state.discover_detail_cache.clear();
                    state.discover_detail_loading.clear();
                    state.discover_detail_errors.clear();
                    state.discover_mode = DiscoverMode::Search;
                    (
                        state.pending_discover_target.clone(),
                        state.discover_detail_navigation_active,
                    )
                };

                let results_len = self.state.borrow().search_results.len();
                if results_len == 0 {
                    self.clear_discover_details(navigation_active);
                    let message = format!("No packages matched \"{}\".", query);
                    self.set_discover_status(Some(&message));
                } else {
                    let message = format!(
                        "Found {} package{} for \"{}\".",
                        results_len,
                        if results_len == 1 { "" } else { "s" },
                        query
                    );
                    self.set_discover_status(Some(&message));
                    self.rebuild_search_list();
                }
                self.update_discover_layout();

                if let Some(target) = pending_target {
                    if !self.focus_discover_package(&target, navigation_active) {
                        let mut state = self.state.borrow_mut();
                        state.pending_discover_target = None;
                        state.discover_detail_navigation_active = false;
                        self.update_discover_detail_back_button();
                    }
                }
            }
            Err(err) => {
                self.clear_search_results();
                let message = format!("Could not search: {}", err);
                self.set_discover_status(Some(&message));
                self.show_toast("Search failed.");
            }
        }
    }

    pub(crate) fn finish_discover_detail(
        self: &Rc<Self>,
        package: String,
        result: Result<DiscoverDetail, String>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.discover_detail_loading.remove(&package);
            match result {
                Ok(detail) => {
                    let cloned = detail.clone();
                    state.discover_detail_errors.remove(&package);
                    state.discover_detail_cache.insert(package.clone(), detail);

                    if let Some(pkg) = state
                        .search_results
                        .iter_mut()
                        .find(|pkg| pkg.name == package)
                    {
                        if let Some(description) = cloned.description.clone() {
                            if !description.is_empty() {
                                pkg.set_description(description);
                            }
                        }
                        if let Some(download) = cloned.download.clone() {
                            pkg.download_size = Some(download);
                        } else if let Some(bytes) = cloned.download_bytes {
                            pkg.download_size = Some(format_size(bytes));
                        }
                        if let Some(bytes) = cloned.download_bytes {
                            pkg.download_bytes = Some(bytes);
                        }
                        if let Some(repo) = cloned.repository.clone() {
                            pkg.repository = Some(repo);
                        }
                    }

                    let installed_flag = state.installed_set.contains(&package);
                    if let Some(focus) = state.discover_detail_focus.as_mut() {
                        if focus.name == package {
                            if let Some(ver) = cloned.version.clone() {
                                focus.set_version(ver);
                            }
                            if let Some(description) = cloned.description.clone() {
                                if !description.is_empty() {
                                    focus.set_description(description);
                                }
                            }
                            focus.download_bytes = cloned.download_bytes;
                            focus.download_size = cloned
                                .download
                                .clone()
                                .or_else(|| cloned.download_bytes.map(format_size));
                            focus.repository = cloned.repository.clone();
                            focus.installed = installed_flag;
                        }
                    }
                }
                Err(err) => {
                    state.discover_detail_errors.insert(package.clone(), err);
                }
            }
        }

        self.update_discover_details();
        self.update_spotlight_recent_detail();
    }

    pub(crate) fn set_discover_status(&self, _message: Option<&str>) {
        // No-op; messages now routed to footer
    }

    pub(crate) fn update_discover_detail_back_button(&self) {
        let has_history = !self.state.borrow().discover_detail_history.is_empty();
        let button = &self.widgets.discover.detail_back_button;
        button.set_visible(has_history);
        button.set_sensitive(has_history);
    }

    pub(crate) fn focus_discover_package(self: &Rc<Self>, package: &str, navigation: bool) -> bool {
        if navigation {
            self.state.borrow_mut().discover_detail_navigation_active = true;
        }

        let position = {
            let state = self.state.borrow();
            state
                .search_results
                .iter()
                .position(|pkg| pkg.name == package)
        };

        let Some(idx) = position else {
            return false;
        };

        {
            let mut state = self.state.borrow_mut();
            state.selected_search = Some(idx);
            state.pending_discover_target = None;
            if let Some(pkg) = state.search_results.get(idx) {
                state.discover_detail_focus = Some(pkg.clone());
            }
        }

        if let Some(row) = self.widgets.discover.list.row_at_index(idx as i32) {
            select_row_if_attached(&self.widgets.discover.list, &row);
        } else {
            self.update_discover_details();
        }

        true
    }

    pub(crate) fn on_discover_dependency_clicked(self: &Rc<Self>, package: String) {
        let package = package.trim();
        if package.is_empty() {
            return;
        }
        let package = package.to_string();

        {
            let mut state = self.state.borrow_mut();
            if let Some(current) = state.discover_detail_package.clone() {
                if current == package {
                    state.discover_detail_navigation_active = false;
                    state.pending_discover_target = None;
                    drop(state);
                    self.update_discover_detail_back_button();
                    return;
                }
                state.discover_detail_history.push(current);
            }
            state.discover_detail_navigation_active = true;
            state.pending_discover_target = Some(package.clone());
        }

        if self.focus_discover_package(&package, true) {
            self.update_discover_detail_back_button();
            return;
        }

        self.open_discover_dependency_detail(package);
    }

    pub(crate) fn open_discover_dependency_detail(self: &Rc<Self>, package: String) {
        let installed = {
            let state = self.state.borrow();
            state.installed_set.contains(&package)
        };

        {
            let mut state = self.state.borrow_mut();
            state.discover_detail_navigation_active = true;
            state.selected_search = None;
            state.pending_discover_target = None;
            let version = String::new();
            let description = String::new();
            state.discover_detail_focus = Some(PackageInfo {
                name_lower: lowercase_cache(&package),
                version_lower: lowercase_cache(&version),
                description_lower: lowercase_cache(&description),
                name: package.clone(),
                version,
                description,
                installed,
                previous_version: None,
                download_size: None,
                changelog: None,
                download_bytes: None,
                repository: None,
                build_date: None,
                first_seen: None,
            });
            state.discover_detail_package = Some(package.clone());
        }

        self.update_discover_details();
        self.update_spotlight_recent_detail();
        self.request_discover_detail(&package);
        self.update_discover_detail_back_button();
    }

    pub(crate) fn set_discover_row_buttons_visible(&self, visible: bool) {
        for button in self.discover_buttons.borrow().iter() {
            button.set_visible(visible);
        }
    }

    pub(crate) fn on_discover_detail_back(self: &Rc<Self>) {
        let previous = {
            let mut state = self.state.borrow_mut();
            state.discover_detail_history.pop()
        };

        if let Some(package) = previous {
            {
                let mut state = self.state.borrow_mut();
                state.discover_detail_navigation_active = true;
                state.pending_discover_target = Some(package.clone());
            }

            if self.focus_discover_package(&package, true) {
                self.update_discover_detail_back_button();
                return;
            }

            self.open_discover_dependency_detail(package);
            return;
        } else {
            self.state.borrow_mut().discover_detail_navigation_active = false;
        }

        self.update_discover_detail_back_button();
    }

    pub(crate) fn rebuild_search_list(self: &Rc<Self>) {
        let list = &self.widgets.discover.list;
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }

        let (results, selected_idx, pending_target, navigation_active) = {
            let state = self.state.borrow();
            (
                state.search_results.clone(),
                state.selected_search,
                state.pending_discover_target.clone(),
                state.discover_detail_navigation_active,
            )
        };
        self.discover_buttons.borrow_mut().clear();
        for pkg in &results {
            let row = self.build_discover_row(pkg);
            list.append(&row);
        }

        if let Some(idx) = selected_idx {
            if let Some(row) = list.row_at_index(idx as i32) {
                select_row_if_attached(list, &row);
            }
        } else if let Some(target) = pending_target {
            let _ = self.focus_discover_package(&target, navigation_active);
        } else {
            list.unselect_all();
        }
        self.update_discover_details();
    }

    pub(crate) fn build_discover_row(self: &Rc<Self>, pkg: &PackageInfo) -> adw::ActionRow {
        let title = glib::markup_escape_text(&pkg.name);
        let version_line = if pkg.version.is_empty() {
            "Version —".to_string()
        } else {
            format!("Version {}", glib::markup_escape_text(&pkg.version))
        };
        let description = if pkg.description.is_empty() {
            "No description provided".to_string()
        } else {
            glib::markup_escape_text(&pkg.description).to_string()
        };

        let subtitle = format!("{}\n{}", version_line, description);

        let row = adw::ActionRow::builder()
            .title(title.as_str())
            .subtitle(subtitle.as_str())
            .build();
        row.set_activatable(false);
        row.set_focusable(false);
        row.set_title_lines(1);
        row.set_subtitle_lines(2);

        let icon = gtk::Image::from_resource(icon_resource_for_package(&pkg.name));
        icon.set_pixel_size(32);
        icon.set_margin_end(12);
        icon.set_valign(gtk::Align::Center);
        row.add_prefix(&icon);

        let button = gtk::Button::builder().width_request(140).build();
        button.set_valign(gtk::Align::Center);

        {
            let state = self.state.borrow();
            if pkg.installed {
                button.set_label("Installed");
                button.add_css_class("pill");
                button.set_tooltip_text(Some("Already installed."));
            } else {
                button.set_label("Install");
                button.add_css_class("suggested-action");
                button.set_sensitive(!state.install_in_progress);
                button.set_tooltip_text(Some("Install this package."));
            }
        }

        let package_name = pkg.name.clone();
        let weak_self = Rc::downgrade(self);
        if !pkg.installed {
            button.connect_clicked(move |_| {
                if let Some(controller) = weak_self.upgrade() {
                    controller.select_search_row_by_name(&package_name);
                    controller.on_discover_primary_action();
                }
            });
        }

        row.add_suffix(&button);
        self.discover_buttons.borrow_mut().push(button);

        row
    }

    pub(crate) fn update_discover_layout(&self) {
        let (mode, has_results) = {
            let state = self.state.borrow();
            (state.discover_mode, !state.search_results.is_empty())
        };

        let spotlight_visible = mode == DiscoverMode::Spotlight;
        self.widgets
            .discover
            .spotlight_section_box
            .set_visible(spotlight_visible);
        self.widgets.discover.scroller.set_visible(has_results);
        self.widgets.discover.scroller.set_vexpand(has_results);
        self.widgets.discover.content_row.set_visible(has_results);
        self.widgets.discover.content_row.set_vexpand(has_results);
    }

    pub(crate) fn clear_search_results(self: &Rc<Self>) {
        let mut state = self.state.borrow_mut();
        state.search_results.clear();
        state.selected_search = None;
        state.discover_mode = DiscoverMode::Spotlight;
        state.discover_detail_cache.clear();
        state.discover_detail_loading.clear();
        state.discover_detail_errors.clear();
        state.discover_detail_focus = None;
        drop(state);
        self.rebuild_search_list();
        self.clear_discover_details(false);
        self.update_discover_layout();
    }

    pub(crate) fn update_search_installed_flags(self: &Rc<Self>) {
        {
            let state = self.state.borrow();
            if state.search_results.is_empty() {
                return;
            }
        }

        {
            let installed_set = self.state.borrow().installed_set.clone();
            let mut state = self.state.borrow_mut();
            for pkg in &mut state.search_results {
                pkg.installed = installed_set.contains(&pkg.name);
            }
        }
        self.rebuild_search_list();
    }

    pub(crate) fn update_discover_details(self: &Rc<Self>) {
        let stack = &self.widgets.discover.detail_stack;
        let button = &self.widgets.discover.detail_action_button;
        let version_value = &self.widgets.discover.detail_version_value;
        let download_value = &self.widgets.discover.detail_download_value;
        let homepage_row = &self.widgets.discover.detail_homepage_row;
        let homepage_link = &self.widgets.discover.detail_homepage_link;
        let maintainer_row = &self.widgets.discover.detail_maintainer_row;
        let maintainer_value = &self.widgets.discover.detail_maintainer_value;
        let license_row = &self.widgets.discover.detail_license_row;
        let license_value = &self.widgets.discover.detail_license_value;
        let update_label = &self.widgets.discover.detail_update_label;
        let description = &self.widgets.discover.detail_description;
        let dependencies_stack = &self.widgets.discover.detail_dependencies_stack;
        let dependencies_list = &self.widgets.discover.detail_dependencies_list;
        let dependencies_placeholder = &self.widgets.discover.detail_dependencies_placeholder;

        let (pkg, detail, loading, error, install_in_progress, remove_in_progress) = {
            let state = self.state.borrow();
            let focus = state.discover_detail_focus.clone();
            let detail = focus
                .as_ref()
                .and_then(|pkg| state.discover_detail_cache.get(&pkg.name).cloned());
            let loading = focus.as_ref().map_or(false, |pkg| {
                state.discover_detail_loading.contains(&pkg.name)
            });
            let error = focus
                .as_ref()
                .and_then(|pkg| state.discover_detail_errors.get(&pkg.name).cloned());
            (
                focus,
                detail,
                loading,
                error,
                state.install_in_progress,
                state.remove_in_progress,
            )
        };

        if let Some(pkg) = pkg {
            stack.set_visible_child_name("detail");
            self.widgets.discover.detail_frame.set_visible(true);
            self.widgets.discover.detail_close_button.set_visible(true);
            self.widgets
                .discover
                .detail_close_button
                .set_sensitive(true);
            self.widgets.discover.detail_name.set_text(&pkg.name);

            let actions_enabled = !loading && !install_in_progress && !remove_in_progress;
            button.set_visible(true);
            button.set_sensitive(actions_enabled);
            button.remove_css_class("suggested-action");
            button.remove_css_class("destructive-action");
            if pkg.installed {
                button.set_label("Remove");
                button.add_css_class("destructive-action");
            } else {
                button.set_label("Install");
                button.add_css_class("suggested-action");
            }

            if loading {
                update_label.set_visible(false);
                update_label.set_text("");
                homepage_row.set_visible(false);
                set_link_label(homepage_link, None);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                license_row.set_visible(false);
                license_value.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or(detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Loading dependencies…");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description.set_text("Loading package details…");
            } else if let Some(error) = error.clone() {
                update_label.set_visible(false);
                update_label.set_text("");
                homepage_row.set_visible(false);
                set_link_label(homepage_link, None);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                license_row.set_visible(false);
                license_value.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or(detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Dependency information unavailable.");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description.set_text(&format!("Could not load package details: {}", error));
            } else {
                let version_text = detail
                    .as_ref()
                    .and_then(|d| d.version.clone())
                    .filter(|v| !v.is_empty())
                    .or_else(|| (!pkg.version.is_empty()).then(|| pkg.version.clone()))
                    .unwrap_or_else(|| "—".to_string());
                version_value.set_text(&version_text);

                set_download_label(
                    download_value,
                    detail.as_ref().and_then(|d| d.download_bytes),
                    detail.as_ref().and_then(|d| d.download.as_deref()),
                    pkg.download_bytes,
                    pkg.download_size.as_deref(),
                );

                if let Some(detail) = detail.as_ref() {
                    if let Some(homepage) = detail.homepage.as_deref() {
                        homepage_row.set_visible(true);
                        set_link_label(homepage_link, Some(homepage));
                    } else {
                        homepage_row.set_visible(false);
                        set_link_label(homepage_link, None);
                    }

                    if let Some(maintainer) = detail.maintainer.as_deref() {
                        let friendly = sanitize_contact_field(maintainer);
                        if friendly.is_empty() {
                            maintainer_row.set_visible(false);
                            maintainer_value.set_visible(false);
                            maintainer_value.set_text("");
                        } else {
                            maintainer_row.set_visible(true);
                            maintainer_value.set_visible(true);
                            maintainer_value.set_text(&friendly);
                        }
                    } else {
                        maintainer_row.set_visible(false);
                        maintainer_value.set_visible(false);
                        maintainer_value.set_text("");
                    }

                    if let Some(license) = detail.license.as_deref() {
                        license_row.set_visible(true);
                        license_value.set_visible(true);
                        license_value.set_text(&license);
                    } else {
                        license_row.set_visible(false);
                        license_value.set_visible(false);
                        license_value.set_text("");
                    }

                    if pkg.installed {
                        update_label.set_visible(false);
                        update_label.set_text("");
                    } else if let Some(prev) = pkg.previous_version.clone() {
                        if prev.is_empty() {
                            update_label.set_visible(false);
                            update_label.set_text("");
                        } else {
                            update_label.set_text(&format!("Updated from {}.", prev));
                            update_label.set_visible(true);
                        }
                    } else {
                        update_label.set_visible(false);
                        update_label.set_text("");
                    }

                    clear_listbox(dependencies_list);
                    let installed_set = {
                        let state = self.state.borrow();
                        state.installed_set.clone()
                    };
                    if detail.dependencies.is_empty() {
                        dependencies_placeholder.set_text("No runtime dependencies.");
                        dependencies_list.set_visible(false);
                        dependencies_stack.set_visible_child_name("placeholder");
                    } else {
                        for dependency in &detail.dependencies {
                            let row = adw::ActionRow::builder()
                                .title(dependency.name.as_str())
                                .build();
                            row.set_activatable(true);
                            row.set_focusable(true);

                            let status_text = if installed_set.contains(&dependency.name) {
                                "Installed"
                            } else {
                                "Not installed"
                            };
                            let status_label = gtk::Label::builder()
                                .label(status_text)
                                .halign(gtk::Align::End)
                                .valign(gtk::Align::Center)
                                .build();
                            status_label.add_css_class("dim-label");
                            row.add_suffix(&status_label);

                            let package_name = dependency.name.clone();
                            row.connect_activated(glib::clone!(
                                #[strong(rename_to = controller)]
                                self,
                                move |_| {
                                    controller.on_discover_dependency_clicked(package_name.clone());
                                }
                            ));

                            dependencies_list.append(&row);
                        }
                        dependencies_list.set_visible(true);
                        dependencies_stack.set_visible_child_name("list");
                    }

                    let description_text = detail
                        .description
                        .clone()
                        .unwrap_or_else(|| pkg.description.clone());
                    if description_text.trim().is_empty() {
                        description.set_text("This package does not provide a description.");
                    } else {
                        description.set_text(&description_text);
                    }
                } else {
                    homepage_row.set_visible(false);
                    set_link_label(homepage_link, None);
                    maintainer_row.set_visible(false);
                    maintainer_value.set_visible(false);
                    maintainer_value.set_text("");
                    license_row.set_visible(false);
                    license_value.set_visible(false);
                    license_value.set_text("");
                    update_label.set_visible(false);
                    update_label.set_text("");
                    clear_listbox(dependencies_list);
                    dependencies_placeholder.set_text("Loading dependencies…");
                    dependencies_list.set_visible(false);
                    dependencies_stack.set_visible_child_name("placeholder");
                    description.set_text("Loading package details…");
                    self.request_discover_detail(&pkg.name);
                }
            }

            button.set_visible(true);
            button.set_sensitive(!install_in_progress && !remove_in_progress);
            return;
        }

        stack.set_visible_child_name("placeholder");
        self.widgets.discover.detail_frame.set_visible(false);
        self.widgets.discover.detail_close_button.set_visible(false);
        self.widgets
            .discover
            .detail_close_button
            .set_sensitive(false);
        self.widgets
            .discover
            .detail_name
            .set_text("Select a package");
        description.set_text("Select a package to see details.");
        download_value.set_text("—");
        version_value.set_text("—");
        update_label.set_visible(false);
        update_label.set_text("");
        homepage_row.set_visible(false);
        set_link_label(homepage_link, None);
        maintainer_row.set_visible(false);
        maintainer_value.set_visible(false);
        maintainer_value.set_text("");
        license_row.set_visible(false);
        license_value.set_visible(false);
        license_value.set_text("");
        clear_listbox(dependencies_list);
        dependencies_placeholder.set_text("No runtime dependencies.");
        dependencies_list.set_visible(false);
        dependencies_stack.set_visible_child_name("placeholder");
        self.set_discover_row_buttons_visible(true);
        self.update_discover_detail_back_button();
    }

    pub(crate) fn request_discover_detail(self: &Rc<Self>, package: &str) {
        let package_name = package.to_string();
        {
            let mut state = self.state.borrow_mut();
            if state.discover_detail_cache.contains_key(&package_name)
                || state.discover_detail_loading.contains(&package_name)
            {
                return;
            }
            state.discover_detail_errors.remove(&package_name);
            state.discover_detail_loading.insert(package_name.clone());
        }

        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = crate::helpers::query_discover_detail(&package_name);
            let _ = sender.send(AppMessage::DiscoverDetailLoaded {
                package: package_name,
                result,
            });
        });
    }

    pub(crate) fn clear_discover_details(&self, preserve_navigation: bool) {
        self.set_discover_status(None);
        {
            let mut state = self.state.borrow_mut();
            if !preserve_navigation {
                state.discover_detail_history.clear();
                state.discover_detail_navigation_active = false;
                state.pending_discover_target = None;
            }
            state.discover_detail_package = None;
            state.discover_detail_focus = None;
        }
        self.widgets
            .discover
            .detail_name
            .set_text("Select a package");
        self.widgets
            .discover
            .detail_stack
            .set_visible_child_name("placeholder");
        self.widgets.discover.detail_frame.set_visible(false);
        self.widgets.discover.detail_close_button.set_visible(false);
        self.widgets
            .discover
            .detail_close_button
            .set_sensitive(false);
        self.widgets
            .discover
            .detail_action_button
            .set_visible(false);
        self.widgets
            .discover
            .detail_homepage_link
            .set_visible(false);
        self.widgets
            .discover
            .detail_maintainer_row
            .set_visible(false);
        self.widgets
            .discover
            .detail_maintainer_value
            .set_visible(false);
        self.widgets
            .discover
            .detail_license_value
            .set_visible(false);
        self.widgets
            .discover
            .detail_description
            .set_text("Select a package to see details.");
        clear_listbox(&self.widgets.discover.detail_dependencies_list);
        self.widgets
            .discover
            .detail_dependencies_placeholder
            .set_text("No runtime dependencies.");
        self.widgets
            .discover
            .detail_dependencies_list
            .set_visible(false);
        self.widgets
            .discover
            .detail_dependencies_stack
            .set_visible_child_name("placeholder");
        self.set_discover_row_buttons_visible(true);
        self.update_discover_detail_back_button();
    }

    pub(crate) fn current_search_selection(&self) -> Option<PackageInfo> {
        let state = self.state.borrow();
        if let Some(idx) = state.selected_search {
            if let Some(pkg) = state.search_results.get(idx) {
                return Some(pkg.clone());
            }
        }
        state.discover_detail_focus.clone()
    }

    pub(crate) fn select_search_row_by_name(self: &Rc<Self>, name: &str) {
        let _ = self.focus_discover_package(name, false);
    }

    pub(crate) fn on_discover_detail_close(self: &Rc<Self>) {
        self.widgets.discover.list.unselect_all();
        self.clear_spotlight_recent_selection();
        self.clear_discover_details(false);
    }

    pub(crate) fn finish_spotlight_loaded(
        self: &Rc<Self>,
        recent: Vec<PackageInfo>,
        categories: HashMap<SpotlightCategory, Vec<PackageInfo>>,
        cache: SpotlightCache,
        refreshed_at: DateTime<Utc>,
    ) {
        let cache_snapshot = {
            let mut state = self.state.borrow_mut();
            state.spotlight_loading = false;
            state.spotlight_recent = recent;
            state.spotlight_categories = categories;
            state.spotlight_cache = cache;
            state.spotlight_last_refresh = Some(refreshed_at);
            if let Some(selected) = state.spotlight_recent_selected.clone() {
                if !state
                    .spotlight_recent
                    .iter()
                    .any(|pkg| pkg.name == selected)
                {
                    state.spotlight_recent_selected = None;
                }
            }
            state.spotlight_cache.clone()
        };

        if let Err(err) = save_spotlight_cache_to_disk(&cache_snapshot) {
            eprintln!("Failed to persist spotlight cache: {}", err);
        }

        self.update_spotlight_installed_flags();
        self.update_spotlight_views();
        self.update_discover_layout();
    }

    pub(crate) fn finish_spotlight_failed(self: &Rc<Self>, error: String) {
        {
            let mut state = self.state.borrow_mut();
            state.spotlight_loading = false;
        }

        eprintln!("Spotlight refresh failed: {}", error);
        self.update_spotlight_views();
        self.widgets
            .discover
            .spotlight_status
            .set_text(&format!("Could not refresh spotlight: {}", error));
    }

    pub(crate) fn initialize_spotlight(self: &Rc<Self>) {
        self.update_spotlight_installed_flags();
        self.update_spotlight_views();
        self.update_discover_layout();
        self.maybe_refresh_spotlight(false);
    }

    pub(crate) fn maybe_refresh_spotlight(self: &Rc<Self>, force: bool) {
        let should_refresh = {
            let state = self.state.borrow();
            if state.spotlight_loading {
                return;
            }
            let has_cached_data = !state.spotlight_recent.is_empty();
            if force {
                true
            } else if !has_cached_data {
                true
            } else if let Some(last) = state.spotlight_last_refresh {
                let delta = Utc::now().signed_duration_since(last);
                delta.num_hours() >= SPOTLIGHT_REFRESH_INTERVAL_HOURS
            } else {
                true
            }
        };

        if !should_refresh {
            return;
        }

        {
            let mut state = self.state.borrow_mut();
            state.spotlight_loading = true;
        }
        self.update_spotlight_views();

        let cache = {
            let state = self.state.borrow();
            state.spotlight_cache.clone()
        };
        let sender = self.sender.clone();
        thread::spawn(move || match refresh_spotlight_cache(cache) {
            Ok(outcome) => {
                let _ = sender.send(AppMessage::SpotlightLoaded {
                    recent: outcome.recent,
                    categories: outcome.categories,
                    cache: outcome.cache,
                    refreshed_at: outcome.refreshed_at,
                });
            }
            Err(error) => {
                let _ = sender.send(AppMessage::SpotlightFailed { error });
            }
        });
    }

    pub(crate) fn set_category_button_state(self: &Rc<Self>, active: Option<SpotlightCategory>) {
        let widgets = &self.widgets.discover;
        set_toggle_button_state(
            &widgets.category_browsers_button,
            active == Some(SpotlightCategory::Browsers),
        );
        set_toggle_button_state(
            &widgets.category_chat_button,
            active == Some(SpotlightCategory::Chat),
        );
        set_toggle_button_state(
            &widgets.category_email_button,
            active == Some(SpotlightCategory::Email),
        );
        set_toggle_button_state(
            &widgets.category_games_button,
            active == Some(SpotlightCategory::Games),
        );
        set_toggle_button_state(
            &widgets.category_graphics_button,
            active == Some(SpotlightCategory::Graphics),
        );
        set_toggle_button_state(
            &widgets.category_music_button,
            active == Some(SpotlightCategory::Music),
        );
        set_toggle_button_state(
            &widgets.category_productivity_button,
            active == Some(SpotlightCategory::Productivity),
        );
        set_toggle_button_state(
            &widgets.category_utilities_button,
            active == Some(SpotlightCategory::Utilities),
        );
        set_toggle_button_state(
            &widgets.category_video_button,
            active == Some(SpotlightCategory::Video),
        );
    }

    pub(crate) fn update_spotlight_installed_flags(self: &Rc<Self>) {
        let installed = {
            let state = self.state.borrow();
            state.installed_set.clone()
        };

        let mut needs_rebuild = false;

        {
            let mut state = self.state.borrow_mut();
            for pkg in &mut state.spotlight_recent {
                let installed_flag = installed.contains(&pkg.name);
                if pkg.installed != installed_flag {
                    pkg.installed = installed_flag;
                    needs_rebuild = true;
                }
            }

            for packages in state.spotlight_categories.values_mut() {
                for pkg in packages.iter_mut() {
                    let installed_flag = installed.contains(&pkg.name);
                    if pkg.installed != installed_flag {
                        pkg.installed = installed_flag;
                        needs_rebuild = true;
                    }
                }
            }
        }

        if needs_rebuild {
            self.rebuild_search_list();
        }
    }

    pub(crate) fn apply_spotlight_category(
        self: &Rc<Self>,
        category: SpotlightCategory,
        store_backup: bool,
    ) {
        let status_snapshot = if store_backup {
            Some(self.widgets.discover.status_label.text().to_string())
        } else {
            None
        };

        let packages = {
            let state = self.state.borrow();
            state
                .spotlight_categories
                .get(&category)
                .cloned()
                .unwrap_or_default()
        };

        {
            let mut state = self.state.borrow_mut();
            if store_backup && state.spotlight_search_backup.is_none() {
                state.spotlight_search_backup = Some(state.search_results.clone());
                state.spotlight_status_backup = status_snapshot;
            }
            state.active_spotlight_category = Some(category);
            state.search_results = packages;
            state.selected_search = None;
            state.discover_mode = DiscoverMode::Spotlight;
            state.discover_detail_focus = None;
        }

        self.set_category_button_state(Some(category));

        let count = {
            let state = self.state.borrow();
            state.search_results.len()
        };

        let label = if count == 0 {
            format!(
                "No spotlight picks found for {}.",
                category_display_name(category)
            )
        } else {
            format!(
                "Showing {} {} spotlight pick{}.",
                count,
                category_display_name(category).to_lowercase(),
                if count == 1 { "" } else { "s" }
            )
        };

        self.rebuild_search_list();
        self.clear_discover_details(false);
        self.set_discover_status(Some(&label));
        self.update_discover_layout();
    }

    pub(crate) fn clear_spotlight_category(self: &Rc<Self>) {
        let (backup_results, backup_status) = {
            let mut state = self.state.borrow_mut();
            state.active_spotlight_category = None;
            (
                state.spotlight_search_backup.take(),
                state.spotlight_status_backup.take(),
            )
        };

        self.set_category_button_state(None);

        if let Some(results) = backup_results {
            {
                let mut state = self.state.borrow_mut();
                state.search_results = results;
                state.selected_search = None;
                state.discover_mode = DiscoverMode::Spotlight;
                state.discover_detail_focus = None;
            }
            self.rebuild_search_list();
            self.clear_discover_details(false);
            if let Some(status) = backup_status {
                self.set_discover_status(Some(&status));
            }
        } else {
            {
                let mut state = self.state.borrow_mut();
                state.search_results.clear();
                state.selected_search = None;
                state.discover_mode = DiscoverMode::Spotlight;
                state.discover_detail_cache.clear();
                state.discover_detail_loading.clear();
                state.discover_detail_errors.clear();
                state.discover_detail_focus = None;
            }
            self.rebuild_search_list();
            self.clear_discover_details(false);
        }

        self.update_discover_layout();
    }

    pub(crate) fn refresh_active_spotlight_category(self: &Rc<Self>) {
        let category = {
            let state = self.state.borrow();
            state.active_spotlight_category
        };

        if let Some(category) = category {
            self.apply_spotlight_category(category, false);
        }
    }

    pub(crate) fn handle_spotlight_category_toggle(
        self: &Rc<Self>,
        category: SpotlightCategory,
        active: bool,
    ) {
        if active {
            self.apply_spotlight_category(category, true);
            self.run_category_search(category);
        } else {
            let should_clear = {
                let state = self.state.borrow();
                state.active_spotlight_category == Some(category)
            };
            if should_clear {
                self.clear_spotlight_category();
            }
        }
    }

    pub(crate) fn run_category_search(self: &Rc<Self>, category: SpotlightCategory) {
        let query = match category {
            SpotlightCategory::Browsers => "web browser",
            SpotlightCategory::Chat => "chat",
            SpotlightCategory::Games => "game",
            SpotlightCategory::Email => "email",
            SpotlightCategory::Graphics => "graphics",
            SpotlightCategory::Music => "music",
            SpotlightCategory::Productivity => "productivity",
            SpotlightCategory::Utilities => "utility",
            SpotlightCategory::Video => "video",
        };

        self.widgets.discover.search_entry.set_text(query);
        self.on_search_requested();
    }

    pub(crate) fn on_spotlight_recent_selected(self: &Rc<Self>, row: Option<gtk::ListBoxRow>) {
        let Some(row) = row else {
            return;
        };
        if !row.is_selected() {
            return;
        }
        self.activate_spotlight_recent_row(&row);
    }

    pub(crate) fn on_spotlight_row_activated(self: &Rc<Self>, row: &gtk::ListBoxRow) {
        self.activate_spotlight_recent_row(row);
    }

    pub(crate) fn activate_spotlight_recent_row(self: &Rc<Self>, row: &gtk::ListBoxRow) {
        let name = row
            .child()
            .and_then(|child| child.downcast::<adw::ActionRow>().ok())
            .map(|action_row| action_row.title().to_string())
            .unwrap_or_default();

        let index = row.index();
        if index < 0 {
            if !name.is_empty() {
                self.widgets.discover.search_entry.set_text(&name);
            }
            self.on_search_requested();
            return;
        }
        let pkg = {
            let state = self.state.borrow();
            state.spotlight_recent.get(index as usize).cloned()
        };

        if let Some(pkg) = pkg {
            {
                let mut state = self.state.borrow_mut();
                state.spotlight_recent_selected = Some(pkg.name.clone());
                state.discover_detail_focus = Some(pkg.clone());
                state.discover_detail_package = Some(pkg.name.clone());
                state.discover_detail_navigation_active = false;
                state.discover_detail_history.clear();
                state.pending_discover_target = None;
            }
            self.request_discover_detail(&pkg.name);
            self.update_spotlight_recent_detail();
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("list");
            self.widgets
                .discover
                .spotlight_recent_scroller
                .set_visible(true);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_reveal_child(true);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_can_target(true);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_visible(true);
            self.widgets
                .discover
                .spotlight_recent_close_button
                .set_visible(true);
            self.widgets
                .discover
                .spotlight_recent_close_button
                .set_sensitive(true);
            self.widgets
                .discover
                .spotlight_recent_detail_container
                .set_visible(true);
            self.update_discover_details();
            return;
        }

        self.on_search_requested();
    }

    pub(crate) fn on_spotlight_recent_close(self: &Rc<Self>) {
        self.clear_spotlight_recent_selection();
    }

    pub(crate) fn clear_spotlight_recent_selection(self: &Rc<Self>) {
        {
            let mut state = self.state.borrow_mut();
            if state.spotlight_recent_selected.take().is_some() {
                state.discover_detail_focus = None;
                state.discover_detail_package = None;
                state.discover_detail_navigation_active = false;
                state.discover_detail_history.clear();
                state.pending_discover_target = None;
            }
        }

        self.widgets.discover.spotlight_recent_list.unselect_all();
        self.widgets.discover.spotlight_recent_detail_spinner.stop();
        self.widgets
            .discover
            .spotlight_recent_detail_spinner
            .set_visible(false);

        self.widgets
            .discover
            .spotlight_recent_detail_revealer
            .set_reveal_child(false);
        self.widgets
            .discover
            .spotlight_recent_detail_revealer
            .set_can_target(false);
        self.widgets
            .discover
            .spotlight_recent_detail_revealer
            .set_visible(false);
        self.widgets
            .discover
            .spotlight_recent_close_button
            .set_visible(false);
        self.widgets
            .discover
            .spotlight_recent_close_button
            .set_sensitive(false);
        self.widgets
            .discover
            .spotlight_recent_detail_container
            .set_visible(false);

        let has_items = {
            let state = self.state.borrow();
            !state.spotlight_recent.is_empty()
        };

        self.widgets
            .discover
            .spotlight_recent_stack
            .set_visible_child_name(if has_items { "list" } else { "placeholder" });

        self.update_spotlight_recent_detail();
        self.update_discover_details();
    }

    pub(crate) fn update_spotlight_views(self: &Rc<Self>) {
        let (recent, loading, last_refresh, active_category, selected_recent) = {
            let state = self.state.borrow();
            (
                state.spotlight_recent.clone(),
                state.spotlight_loading,
                state.spotlight_last_refresh,
                state.active_spotlight_category,
                state.spotlight_recent_selected.clone(),
            )
        };

        self.set_category_button_state(active_category);

        let spinner = &self.widgets.discover.spotlight_spinner;
        let status_label = &self.widgets.discover.spotlight_status;

        self.widgets
            .discover
            .spotlight_refresh_button
            .set_sensitive(!loading);

        if loading {
            spinner.set_visible(true);
            spinner.start();
            status_label.set_text("Refreshing spotlight…");
        } else {
            spinner.stop();
            spinner.set_visible(false);
            let status_text = if let Some(last) = last_refresh {
                format!("Last updated {}.", format_relative_time(last))
            } else if recent.is_empty() {
                "Last updated —".to_string()
            } else {
                "Last updated just now.".to_string()
            };
            status_label.set_text(&status_text);
        }

        populate_spotlight_list(&self.widgets.discover.spotlight_recent_list, &recent);
        let detail_available = selected_recent
            .as_ref()
            .map(|name| recent.iter().any(|pkg| pkg.name == *name))
            .unwrap_or(false);

        if selected_recent.is_some() && !detail_available {
            let mut state = self.state.borrow_mut();
            state.spotlight_recent_selected = None;
        }

        if detail_available {
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("list");
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_reveal_child(true);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_can_target(true);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_visible(true);
            self.widgets
                .discover
                .spotlight_recent_close_button
                .set_visible(true);
            self.widgets
                .discover
                .spotlight_recent_close_button
                .set_sensitive(true);
            self.widgets
                .discover
                .spotlight_recent_detail_container
                .set_visible(true);
            self.update_spotlight_recent_detail();
        } else if recent.is_empty() {
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("placeholder");
            self.widgets
                .discover
                .spotlight_recent_scroller
                .set_visible(false);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_reveal_child(false);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_can_target(false);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_visible(false);
            self.widgets
                .discover
                .spotlight_recent_close_button
                .set_visible(false);
            self.widgets
                .discover
                .spotlight_recent_close_button
                .set_sensitive(false);
            self.widgets
                .discover
                .spotlight_recent_detail_container
                .set_visible(false);
        } else {
            self.widgets
                .discover
                .spotlight_recent_stack
                .set_visible_child_name("list");
            self.widgets
                .discover
                .spotlight_recent_scroller
                .set_visible(true);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_reveal_child(false);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_can_target(false);
            self.widgets
                .discover
                .spotlight_recent_detail_revealer
                .set_visible(false);
            self.widgets
                .discover
                .spotlight_recent_close_button
                .set_visible(false);
            self.widgets
                .discover
                .spotlight_recent_close_button
                .set_sensitive(false);
            self.widgets
                .discover
                .spotlight_recent_detail_container
                .set_visible(false);
        }
    }

    pub(crate) fn update_spotlight_recent_detail(self: &Rc<Self>) {
        let (
            selected_recent,
            focus_pkg,
            detail,
            loading,
            error,
            install_in_progress,
            remove_in_progress,
            navigation_active,
            has_history,
        ) = {
            let state = self.state.borrow();
            let focus = state.discover_detail_focus.clone();
            let detail = focus
                .as_ref()
                .and_then(|pkg| state.discover_detail_cache.get(&pkg.name).cloned());
            let loading = focus.as_ref().map_or(false, |pkg| {
                state.discover_detail_loading.contains(&pkg.name)
            });
            let error = focus
                .as_ref()
                .and_then(|pkg| state.discover_detail_errors.get(&pkg.name).cloned());
            (
                state.spotlight_recent_selected.clone(),
                focus,
                detail,
                loading,
                error,
                state.install_in_progress,
                state.remove_in_progress,
                state.discover_detail_navigation_active,
                !state.discover_detail_history.is_empty(),
            )
        };

        let widgets = &self.widgets.discover;
        let back_button = &widgets.spotlight_recent_back_button;
        let spinner = &widgets.spotlight_recent_detail_spinner;
        let status_label = &widgets.spotlight_recent_detail_status;
        let update_label = &widgets.spotlight_recent_detail_update_label;
        let version_value = &widgets.spotlight_recent_detail_version_value;
        let download_value = &widgets.spotlight_recent_detail_download_value;
        let homepage_row = &widgets.spotlight_recent_detail_homepage_row;
        let homepage_link = &widgets.spotlight_recent_detail_homepage_link;
        let maintainer_row = &widgets.spotlight_recent_detail_maintainer_row;
        let maintainer_value = &widgets.spotlight_recent_detail_maintainer_value;
        let license_row = &widgets.spotlight_recent_detail_license_row;
        let license_value = &widgets.spotlight_recent_detail_license_value;
        let updated_row = &widgets.spotlight_recent_detail_updated_row;
        let updated_value = &widgets.spotlight_recent_detail_updated_value;
        let description_label = &widgets.spotlight_recent_detail_description;
        let dependencies_stack = &widgets.spotlight_recent_detail_dependencies_stack;
        let dependencies_list = &widgets.spotlight_recent_detail_dependencies_list;
        let dependencies_placeholder = &widgets.spotlight_recent_detail_dependencies_placeholder;
        let action_button = &widgets.spotlight_recent_action_button;

        if let (Some(_), Some(pkg)) = (&selected_recent, &focus_pkg) {
            let show_back = navigation_active && has_history;
            back_button.set_visible(show_back);
            back_button.set_sensitive(show_back);
            widgets.spotlight_recent_detail_container.set_visible(true);
            widgets.spotlight_recent_detail_name.set_text(&pkg.name);

            let actions_enabled = !loading && !install_in_progress && !remove_in_progress;
            action_button.set_visible(true);
            action_button.set_sensitive(actions_enabled);
            action_button.remove_css_class("suggested-action");
            action_button.remove_css_class("destructive-action");
            if pkg.installed {
                action_button.set_label("Remove");
                action_button.add_css_class("destructive-action");
            } else {
                action_button.set_label("Install");
                action_button.add_css_class("suggested-action");
            }

            if loading {
                spinner.set_visible(true);
                spinner.start();
                status_label.set_visible(true);
                status_label.set_text("Loading details…");
            } else {
                spinner.stop();
                spinner.set_visible(false);
                if let Some(error) = error.clone() {
                    status_label.set_visible(true);
                    status_label.set_text(&format!("Could not load additional details: {}", error));
                } else {
                    status_label.set_visible(false);
                    status_label.set_text("");
                }
            }

            let version_text = detail
                .as_ref()
                .and_then(|d| d.version.clone())
                .filter(|v| !v.is_empty())
                .or_else(|| (!pkg.version.is_empty()).then(|| pkg.version.clone()))
                .unwrap_or_else(|| "—".to_string());
            version_value.set_text(&version_text);

            set_download_label(
                download_value,
                detail.as_ref().and_then(|d| d.download_bytes),
                detail.as_ref().and_then(|d| d.download.as_deref()),
                pkg.download_bytes,
                pkg.download_size.as_deref(),
            );

            if let Some(detail) = detail.clone() {
                if let Some(homepage) = detail.homepage {
                    homepage_row.set_visible(true);
                    set_link_label(homepage_link, Some(homepage.as_str()));
                } else {
                    homepage_row.set_visible(false);
                    set_link_label(homepage_link, None);
                }

                if let Some(maintainer) = detail.maintainer {
                    let friendly = sanitize_contact_field(&maintainer);
                    if friendly.is_empty() {
                        maintainer_row.set_visible(false);
                        maintainer_value.set_visible(false);
                        maintainer_value.set_text("");
                    } else {
                        maintainer_row.set_visible(true);
                        maintainer_value.set_visible(true);
                        maintainer_value.set_text(&friendly);
                    }
                } else {
                    maintainer_row.set_visible(false);
                    maintainer_value.set_visible(false);
                    maintainer_value.set_text("");
                }

                if let Some(license) = detail.license {
                    license_row.set_visible(true);
                    license_value.set_visible(true);
                    license_value.set_text(&license);
                } else {
                    license_row.set_visible(false);
                    license_value.set_visible(false);
                    license_value.set_text("");
                }

                if let Some(updated) = pkg.build_date {
                    updated_row.set_visible(true);
                    updated_value.set_visible(true);
                    updated_value.set_text(&format_relative_time(updated));
                } else {
                    updated_row.set_visible(false);
                    updated_value.set_visible(false);
                    updated_value.set_text("");
                }

                if pkg.installed {
                    update_label.set_visible(false);
                    update_label.set_text("");
                } else {
                    update_label.set_visible(false);
                    update_label.set_text("");
                }

                let description = detail
                    .description
                    .clone()
                    .unwrap_or_else(|| pkg.description.clone());
                if description.trim().is_empty() {
                    description_label.set_text("This package does not provide a description.");
                } else {
                    description_label.set_text(&description);
                }

                clear_listbox(dependencies_list);
                let installed_set = {
                    let state = self.state.borrow();
                    state.installed_set.clone()
                };
                if detail.dependencies.is_empty() {
                    dependencies_placeholder.set_text("No runtime dependencies.");
                    dependencies_list.set_visible(false);
                    dependencies_stack.set_visible_child_name("placeholder");
                } else {
                    for dependency in &detail.dependencies {
                        let row = adw::ActionRow::builder()
                            .title(dependency.name.as_str())
                            .build();
                        row.set_activatable(true);
                        row.set_focusable(true);

                        let status_text = if installed_set.contains(&dependency.name) {
                            "Installed"
                        } else {
                            "Not installed"
                        };
                        let status_label = gtk::Label::builder()
                            .label(status_text)
                            .halign(gtk::Align::End)
                            .valign(gtk::Align::Center)
                            .build();
                        status_label.add_css_class("dim-label");
                        row.add_suffix(&status_label);

                        let package_name = dependency.name.clone();
                        row.connect_activated(glib::clone!(
                            #[strong(rename_to = controller)]
                            self,
                            move |_| {
                                controller.on_discover_dependency_clicked(package_name.clone());
                            }
                        ));

                        dependencies_list.append(&row);
                    }
                    dependencies_list.set_visible(true);
                    dependencies_stack.set_visible_child_name("list");
                }
            } else if loading {
                homepage_row.set_visible(false);
                set_link_label(homepage_link, None);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                maintainer_value.set_text("");
                license_row.set_visible(false);
                license_value.set_visible(false);
                license_value.set_text("");
                updated_row.set_visible(false);
                updated_value.set_visible(false);
                updated_value.set_text("");
                update_label.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or_else(|| detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Loading dependencies…");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description_label.set_text("Loading package details…");
            } else if let Some(err) = error.clone() {
                homepage_row.set_visible(false);
                set_link_label(homepage_link, None);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                maintainer_value.set_text("");
                license_row.set_visible(false);
                license_value.set_visible(false);
                license_value.set_text("");
                updated_row.set_visible(false);
                updated_value.set_visible(false);
                updated_value.set_text("");
                update_label.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or_else(|| detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Dependency information unavailable.");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description_label.set_text(&format!("Could not load package details: {}", err));
            } else {
                homepage_row.set_visible(false);
                set_link_label(homepage_link, None);
                maintainer_row.set_visible(false);
                maintainer_value.set_visible(false);
                maintainer_value.set_text("");
                license_row.set_visible(false);
                license_value.set_visible(false);
                license_value.set_text("");
                updated_row.set_visible(false);
                updated_value.set_visible(false);
                updated_value.set_text("");
                update_label.set_visible(false);
                let fallback_bytes = pkg.download_bytes.or_else(|| detail_download_bytes(&pkg.name));
                set_download_label(
                    download_value,
                    None,
                    None,
                    fallback_bytes,
                    pkg.download_size.as_deref(),
                );
                clear_listbox(dependencies_list);
                dependencies_placeholder.set_text("Loading dependencies…");
                dependencies_list.set_visible(false);
                dependencies_stack.set_visible_child_name("placeholder");
                description_label.set_text("Loading package details…");
                self.request_discover_detail(&pkg.name);
            }
        } else {
            back_button.set_visible(false);
            back_button.set_sensitive(false);
            widgets.spotlight_recent_detail_container.set_visible(false);
            spinner.stop();
            spinner.set_visible(false);
            status_label.set_visible(false);
            status_label.set_text("");
            widgets
                .spotlight_recent_detail_name
                .set_text("Select a package");
            version_value.set_text("—");
            download_value.set_text("—");
            homepage_row.set_visible(false);
            set_link_label(homepage_link, None);
            maintainer_row.set_visible(false);
            maintainer_value.set_visible(false);
            maintainer_value.set_text("");
            license_row.set_visible(false);
            license_value.set_visible(false);
            license_value.set_text("");
            updated_row.set_visible(false);
            updated_value.set_visible(false);
            updated_value.set_text("");
            update_label.set_visible(false);
            description_label.set_text("Select a package to see details.");
            clear_listbox(dependencies_list);
            dependencies_placeholder.set_text("No runtime dependencies.");
            dependencies_list.set_visible(false);
            dependencies_stack.set_visible_child_name("placeholder");
            action_button.set_visible(false);
            action_button.remove_css_class("suggested-action");
            action_button.remove_css_class("destructive-action");
        }
    }
}
