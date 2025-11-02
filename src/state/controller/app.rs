use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use gtk::glib::{self, Propagation};
use gtk::pango;

use crate::settings::{AppSettings, StartPagePreference, UpdateCheckFrequency, save_app_settings};
use crate::spotlight::{
    SpotlightCategory, build_category_results, compute_spotlight_sections,
    load_spotlight_cache_from_disk,
};
use crate::state::types::{AppMessage, AppState, InstalledFilter, RemoveOrigin};
use crate::types::{CommandResult, PackageInfo};
use crate::ui::AppWidgets;
use crate::xbps::{run_xbps_install, run_xbps_remove, run_xbps_remove_packages};
use chrono::Utc;

pub(crate) struct AppController {
    pub(crate) widgets: AppWidgets,
    pub(crate) state: RefCell<AppState>,
    pub(crate) sender: mpsc::Sender<AppMessage>,
    pub(crate) app: adw::Application,
    pub(crate) window: adw::ApplicationWindow,
    pub(crate) settings: Rc<RefCell<AppSettings>>,
    pub(crate) update_buttons: RefCell<Vec<gtk::Button>>,
    pub(crate) installed_buttons: RefCell<Vec<gtk::Button>>,
    pub(crate) discover_buttons: RefCell<Vec<gtk::Button>>,
    pub(crate) preferences_window: RefCell<Option<adw::PreferencesWindow>>,
    pub(crate) about_dialog: RefCell<Option<gtk::Dialog>>,
}

impl AppController {
    pub(crate) fn new(
        widgets: AppWidgets,
        sender: mpsc::Sender<AppMessage>,
        app: adw::Application,
        window: adw::ApplicationWindow,
        settings: Rc<RefCell<AppSettings>>,
    ) -> Self {
        let mut state = AppState::default();
        let cache = load_spotlight_cache_from_disk();
        let now = Utc::now();
        let recent = compute_spotlight_sections(&cache, now);
        let categories = build_category_results(&cache);

        state.spotlight_cache = cache;
        state.spotlight_recent = recent;
        state.spotlight_categories = categories;
        state.spotlight_last_refresh = state.spotlight_cache.generated_at;
        {
            let settings_ref = settings.borrow();
            state.auto_check_enabled = settings_ref.auto_check_enabled;
            state.auto_check_frequency = settings_ref.auto_check_frequency;
            state.confirm_install = settings_ref.confirm_install;
            state.confirm_remove = settings_ref.confirm_remove;
            state.start_page_preference = settings_ref.start_page;
            state.notify_updates = settings_ref.notify_updates;
        }

        Self {
            widgets,
            sender,
            app,
            state: RefCell::new(state),
            window,
            settings,
            update_buttons: RefCell::new(Vec::new()),
            installed_buttons: RefCell::new(Vec::new()),
            discover_buttons: RefCell::new(Vec::new()),
            preferences_window: RefCell::new(None),
            about_dialog: RefCell::new(None),
        }
    }

    pub(crate) fn setup_connections(self: &Rc<Self>) {
        self.widgets.discover.search_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_search_requested();
            }),
        );

        self.widgets.discover.search_entry.connect_search_changed(
            glib::clone!(@strong self as controller => move |entry| {
                controller.on_discover_search_changed(entry.text().to_string());
            }),
        );

        self.widgets.discover.search_entry.connect_activate(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_search_requested();
            }),
        );

        self.widgets.discover.list.connect_row_selected(
            glib::clone!(@strong self as controller => move |_, row| {
                controller.on_search_row_selected(row.cloned());
            }),
        );
        self.widgets.discover.list.connect_row_activated(
            glib::clone!(@strong self as controller => move |_, _| {
                controller.on_discover_primary_action();
            }),
        );
        self.widgets.discover.detail_action_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_discover_primary_action();
            }),
        );
        self.widgets.discover.detail_back_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_discover_detail_back();
            }),
        );
        self.widgets.discover.detail_close_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_discover_detail_close();
            }),
        );
        self.widgets
            .discover
            .spotlight_recent_list
            .connect_row_selected(glib::clone!(@strong self as controller => move |_, row| {
                controller.on_spotlight_recent_selected(row.cloned());
            }));
        self.widgets
            .discover
            .spotlight_recent_list
            .connect_row_activated(glib::clone!(@strong self as controller => move |_, row| {
                controller.on_spotlight_row_activated(row);
            }));
        self.widgets
            .discover
            .spotlight_recent_back_button
            .connect_clicked(glib::clone!(@strong self as controller => move |_| {
                controller.on_spotlight_recent_back();
            }));
        self.widgets
            .discover
            .spotlight_recent_action_button
            .connect_clicked(glib::clone!(@strong self as controller => move |_| {
                controller.on_discover_primary_action();
            }));
        self.widgets
            .discover
            .category_browsers_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Browsers,
                    btn.is_active(),
                );
            }));
        self.widgets
            .discover
            .category_chat_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(SpotlightCategory::Chat, btn.is_active());
            }));
        self.widgets
            .discover
            .category_games_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(SpotlightCategory::Games, btn.is_active());
            }));
        self.widgets
            .discover
            .category_graphics_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Graphics,
                    btn.is_active(),
                );
            }));
        self.widgets.discover.category_email_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Email,
                    btn.is_active(),
                );
            }),
        );
        self.widgets.discover.category_music_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Music,
                    btn.is_active(),
                );
            }),
        );
        self.widgets
            .discover
            .category_productivity_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Productivity,
                    btn.is_active(),
                );
            }));
        self.widgets
            .discover
            .category_utilities_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(
                    SpotlightCategory::Utilities,
                    btn.is_active(),
                );
            }));
        self.widgets
            .discover
            .category_video_button
            .connect_toggled(glib::clone!(@strong self as controller => move |btn| {
                controller.handle_spotlight_category_toggle(SpotlightCategory::Video, btn.is_active());
            }));

        self.widgets
            .discover
            .spotlight_refresh_button
            .connect_clicked(glib::clone!(@strong self as controller => move |_| {
                controller.maybe_refresh_spotlight(true);
            }));

        self.widgets.installed.refresh_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.refresh_installed_packages();
            }),
        );

        self.widgets.installed.search_entry.connect_search_changed(
            glib::clone!(@strong self as controller => move |entry| {
                controller.on_installed_search_changed(entry.text().to_string());
            }),
        );

        self.widgets.installed.search_entry.connect_activate(
            glib::clone!(@strong self as controller => move |entry| {
                controller.on_installed_search_changed(entry.text().to_string());
            }),
        );

        self.widgets.discover_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                if btn.is_active() {
                    controller.switch_to_page("discover");
                }
            }),
        );

        self.widgets.installed_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                if btn.is_active() {
                    controller.switch_to_page("installed");
                }
            }),
        );

        self.widgets.updates_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                if btn.is_active() {
                    controller.switch_to_page("updates");
                }
            }),
        );

        self.widgets.tools_button.connect_toggled(
            glib::clone!(@strong self as controller => move |btn| {
                if btn.is_active() {
                    controller.switch_to_page("tools");
                }
            }),
        );

        self.widgets.tools.cleanup_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_cleanup_requested();
            }),
        );
        self.widgets.tools.pkgdb_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_pkgdb_requested();
            }),
        );
        self.widgets.tools.reconfigure_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_reconfigure_requested();
            }),
        );
        self.widgets.tools.alternatives_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_alternatives_requested();
            }),
        );

        self.widgets
            .installed
            .filter_dropdown
            .connect_selected_notify(glib::clone!(@strong self as controller => move |dropdown| {
                controller.on_installed_filter_changed(dropdown.selected());
            }));

        self.widgets.installed.list.connect_row_selected(
            glib::clone!(@strong self as controller => move |_, row| {
                controller.on_installed_row_selected(row.cloned());
            }),
        );

        self.widgets
            .installed
            .remove_selected_button
            .connect_clicked(glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_remove_selected();
            }));

        self.widgets.installed.detail_back_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_detail_back();
            }),
        );
        self.widgets.installed.detail_close_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_detail_close();
            }),
        );

        self.widgets.installed.detail_remove_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_detail_remove();
            }),
        );

        self.widgets.installed.detail_update_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_installed_detail_update();
            }),
        );

        {
            let state = self.state.borrow();
            let filter_index = match state.installed_filter_mode {
                InstalledFilter::All => 0,
                InstalledFilter::Updates => 1,
            };
            self.widgets
                .installed
                .filter_dropdown
                .set_selected(filter_index);
        }

        self.update_installed_summary();
        self.update_installed_selection_ui();
        self.update_installed_details();

        self.widgets.updates.check_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.refresh_updates(false);
            }),
        );
        self.widgets.updates.refresh_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.refresh_updates(false);
            }),
        );

        self.widgets.updates.update_all_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.update_all_packages();
            }),
        );

        self.widgets.updates.list.connect_row_activated(
            glib::clone!(@strong self as controller => move |_, row| {
                controller.on_update_row_activated(row);
            }),
        );
        self.widgets.updates.detail_close_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_updates_detail_close();
            }),
        );
        self.widgets.updates.detail_update_button.connect_clicked(
            glib::clone!(@strong self as controller => move |_| {
                controller.on_updates_detail_update();
            }),
        );

        self.widgets.updates.list.connect_row_selected(
            glib::clone!(@strong self as controller => move |_, row| {
                controller.on_update_row_selected(row.cloned());
            }),
        );

        let auto_enabled = self.state.borrow().auto_check_enabled;
        if auto_enabled {
            self.schedule_auto_check();
        }

        let weak_self = Rc::downgrade(self);
        self.widgets
            .view_stack
            .connect_visible_child_name_notify(move |_| {
                if let Some(controller) = weak_self.upgrade() {
                    controller.on_view_changed();
                }
            });

        self.update_footer_text();
        self.update_updates_badge();
        self.update_tools_actions();
        self.on_view_changed();
    }

    pub(crate) fn persist_settings(&self) {
        if let Err(err) = save_app_settings(&self.settings.borrow()) {
            eprintln!("Failed to save settings: {}", err);
        }
    }

    pub(crate) fn apply_start_page_preference(&self) {
        let (preference, last_page) = {
            let settings = self.settings.borrow();
            (
                settings.start_page,
                settings
                    .last_page
                    .clone()
                    .unwrap_or_else(|| "discover".to_string()),
            )
        };
        {
            let mut state = self.state.borrow_mut();
            state.start_page_preference = preference;
        }
        self.set_active_page(match preference {
            StartPagePreference::Discover => "discover",
            StartPagePreference::LastVisited => last_page.as_str(),
        });
    }

    pub(crate) fn set_active_page(&self, page: &str) {
        match page {
            "installed" => {
                if !self.widgets.installed_button.is_active() {
                    self.widgets.installed_button.set_active(true);
                } else {
                    self.switch_to_page("installed");
                }
            }
            "updates" => {
                if !self.widgets.updates_button.is_active() {
                    self.widgets.updates_button.set_active(true);
                } else {
                    self.switch_to_page("updates");
                }
            }
            "tools" => {
                if !self.widgets.tools_button.is_active() {
                    self.widgets.tools_button.set_active(true);
                } else {
                    self.switch_to_page("tools");
                }
            }
            _ => {
                if !self.widgets.discover_button.is_active() {
                    self.widgets.discover_button.set_active(true);
                } else {
                    self.switch_to_page("discover");
                }
            }
        }
    }

    pub(crate) fn switch_to_page(&self, page: &str) {
        if self.widgets.view_stack.visible_child_name().as_deref() != Some(page) {
            self.widgets.view_stack.set_visible_child_name(page);
        }
        self.record_last_page(page);
    }

    pub(crate) fn record_last_page(&self, page: &str) {
        {
            let mut settings = self.settings.borrow_mut();
            settings.last_page = Some(page.to_string());
        }
        self.persist_settings();
    }

    pub(crate) fn update_start_page_preference(&self, preference: StartPagePreference) {
        {
            let mut state = self.state.borrow_mut();
            state.start_page_preference = preference;
        }
        {
            let mut settings = self.settings.borrow_mut();
            settings.start_page = preference;
        }
        self.persist_settings();
    }

    pub(crate) fn set_auto_check_enabled(self: &Rc<Self>, enabled: bool, persist: bool) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.auto_check_enabled == enabled {
                false
            } else {
                state.auto_check_enabled = enabled;
                true
            }
        };

        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.auto_check_enabled = enabled;
            }
            self.persist_settings();
        }

        if enabled {
            self.schedule_auto_check();
        } else if changed {
            self.cancel_auto_check_timer();
        }
    }

    pub(crate) fn set_auto_check_frequency(
        self: &Rc<Self>,
        frequency: UpdateCheckFrequency,
        persist: bool,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.auto_check_frequency = frequency;
        }

        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.auto_check_frequency = frequency;
            }
            self.persist_settings();
        }

        if self.state.borrow().auto_check_enabled {
            self.schedule_auto_check();
        }
    }

    pub(crate) fn set_confirm_install(&self, enabled: bool, persist: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.confirm_install = enabled;
        }
        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.confirm_install = enabled;
            }
            self.persist_settings();
        }
    }

    pub(crate) fn set_confirm_remove(&self, enabled: bool, persist: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.confirm_remove = enabled;
        }
        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.confirm_remove = enabled;
            }
            self.persist_settings();
        }
    }

    pub(crate) fn set_notify_updates(self: &Rc<Self>, enabled: bool, persist: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.notify_updates = enabled;
            state.updates_notification_sent = false;
        }
        self.withdraw_updates_notification();
        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.notify_updates = enabled;
            }
            self.persist_settings();
        }
    }

    pub(crate) fn confirm_action<F>(
        self: &Rc<Self>,
        heading: &str,
        body: &str,
        confirm_label: &str,
        on_confirm: F,
    ) where
        F: FnOnce(&Rc<Self>) + 'static,
    {
        let dialog = gtk::MessageDialog::builder()
            .text(heading)
            .secondary_text(body)
            .message_type(gtk::MessageType::Question)
            .modal(true)
            .build();
        dialog.set_transient_for(Some(&self.window));
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button(confirm_label, gtk::ResponseType::Accept);
        dialog.set_default_response(gtk::ResponseType::Accept);
        let controller_weak = Rc::downgrade(self);
        let callback = Rc::new(RefCell::new(Some(on_confirm)));
        dialog.connect_response(move |dlg, response| {
            dlg.close();
            if response == gtk::ResponseType::Accept {
                if let Some(controller) = controller_weak.upgrade() {
                    if let Some(callback) = callback.borrow_mut().take() {
                        callback(&controller);
                    }
                }
            }
        });
        dialog.present();
    }

    pub(crate) fn begin_install(self: &Rc<Self>, package: PackageInfo) {
        self.execute_install(package);
    }

    pub(crate) fn execute_install(self: &Rc<Self>, package: PackageInfo) {
        {
            let mut state = self.state.borrow_mut();
            if state.install_in_progress {
                return;
            }
            state.install_in_progress = true;
        }

        self.rebuild_search_list();

        let message = format!("Installing \"{}\"…", package.name);
        self.set_footer_message(Some(&message));
        let sender = self.sender.clone();
        let package_name = package.name.clone();
        thread::spawn(move || {
            let result = run_xbps_install(&package_name);
            let _ = sender.send(AppMessage::InstallFinished {
                package: package_name,
                result,
            });
        });
    }

    pub(crate) fn execute_remove(self: &Rc<Self>, package: String, origin: RemoveOrigin) {
        {
            let mut state = self.state.borrow_mut();
            if state.remove_in_progress {
                return;
            }
            state.remove_in_progress = true;
        }

        let message = format!("Removing \"{}\"…", package);
        self.set_footer_message(Some(&message));

        match origin {
            RemoveOrigin::Discover => {}
            RemoveOrigin::Installed => {
                self.set_installed_status_message(Some(message.clone()));
                self.rebuild_installed_list();
            }
        }

        self.rebuild_search_list();

        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = run_xbps_remove(&package);
            let _ = sender.send(AppMessage::RemoveFinished { package, result });
        });
    }

    pub(crate) fn execute_remove_batch(self: &Rc<Self>, packages: Vec<String>) {
        if packages.is_empty() {
            return;
        }

        {
            let mut state = self.state.borrow_mut();
            if state.remove_in_progress {
                return;
            }
            state.remove_in_progress = true;
        }

        self.update_installed_selection_ui();

        let message = format!(
            "Removing {} selected package{}…",
            packages.len(),
            if packages.len() == 1 { "" } else { "s" }
        );
        self.set_installed_status_message(Some(message.clone()));
        self.set_footer_message(Some(&message));

        let sender = self.sender.clone();
        let packages_for_thread = packages.clone();
        thread::spawn(move || {
            let result = run_xbps_remove_packages(&packages_for_thread);
            let _ = sender.send(AppMessage::RemoveBatchFinished {
                packages: packages_for_thread,
                result,
            });
        });
    }

    pub(crate) fn on_view_changed(self: &Rc<Self>) {
        let current = self.widgets.view_stack.visible_child_name();
        match current.as_deref() {
            Some("discover") => if !self.widgets.discover_button.is_active() {},
            Some("installed") => {
                if !self.widgets.installed_button.is_active() {}
                if self.state.borrow().installed_packages.is_empty()
                    && !self.state.borrow().installed_refresh_in_progress
                {
                    self.refresh_installed_packages();
                }
            }
            Some("updates") => if !self.widgets.updates_button.is_active() {},
            Some("tools") => if !self.widgets.tools_button.is_active() {},
            _ => {}
        }

        if let Some(name) = current.as_deref() {
            self.record_last_page(name);
        }
    }

    pub(crate) fn start_remove(self: &Rc<Self>, package: String, origin: RemoveOrigin) {
        if self.state.borrow().confirm_remove {
            let pkg_clone = package.clone();
            let heading = format!("Remove \"{}\"?", package);
            let body = "The package and its data will be removed from this system.";
            self.confirm_action(&heading, body, "Remove", move |controller| {
                controller.begin_remove(pkg_clone.clone(), origin);
            });
            return;
        }

        self.begin_remove(package, origin);
    }

    pub(crate) fn begin_remove(self: &Rc<Self>, package: String, origin: RemoveOrigin) {
        self.execute_remove(package, origin);
    }

    pub(crate) fn handle_message(self: &Rc<Self>, msg: AppMessage) {
        match msg {
            AppMessage::SearchFinished { query, result } => {
                self.finish_search(query, result);
            }
            AppMessage::InstalledFinished { result } => {
                self.finish_installed_refresh(result);
            }
            AppMessage::InstallFinished { package, result } => {
                self.finish_install(package, result);
            }
            AppMessage::RemoveFinished { package, result } => {
                self.finish_remove(package, result);
            }
            AppMessage::RemoveBatchFinished { packages, result } => {
                self.finish_remove_batch(packages, result);
            }
            AppMessage::InstalledDetailsLoaded { package, result } => {
                self.finish_installed_detail(package, result);
            }
            AppMessage::UpdatesDetailLoaded { package, result } => {
                self.finish_updates_detail(package, result);
            }
            AppMessage::UpdatesRefreshed {
                packages,
                success,
                error,
            } => {
                self.finish_updates_refresh(packages, success, error);
            }
            AppMessage::UpdateFinished {
                packages,
                result,
                all,
            } => {
                self.finish_update(packages, result, all);
            }
            AppMessage::DiscoverDetailLoaded { package, result } => {
                self.finish_discover_detail(package, result);
            }
            AppMessage::SpotlightLoaded {
                recent,
                categories,
                cache,
                refreshed_at,
            } => {
                self.finish_spotlight_loaded(recent, categories, cache, refreshed_at);
            }
            AppMessage::SpotlightFailed { error } => {
                self.finish_spotlight_failed(error);
            }
            AppMessage::MaintenanceFinished { task, result } => {
                self.finish_maintenance(task, result);
            }
        }
    }

    pub(crate) fn show_alternatives_dialog(&self, output: &str) {
        let dialog = gtk::Dialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .title("Available alternatives")
            .default_width(520)
            .default_height(420)
            .build();
        dialog.add_button("Close", gtk::ResponseType::Close);
        dialog.connect_response(|dialog, _| dialog.close());

        let content = dialog.content_area();
        content.set_spacing(12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);

        let info_label = gtk::Label::builder()
            .label("Here's what \"xbps-alternatives -l\" reported.")
            .halign(gtk::Align::Start)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .build();
        info_label.add_css_class("dim-label");
        content.append(&info_label);

        let scroller = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .min_content_height(320)
            .build();

        let buffer = gtk::TextBuffer::new(None);
        let trimmed = output.trim();
        if trimmed.is_empty() {
            buffer.set_text("No alternatives were reported.");
        } else {
            buffer.set_text(trimmed);
        }

        let text_view = gtk::TextView::builder()
            .buffer(&buffer)
            .editable(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::None)
            .build();
        text_view.set_cursor_visible(false);

        scroller.set_child(Some(&text_view));
        content.append(&scroller);

        dialog.present();
    }

    pub(crate) fn finish_install(
        self: &Rc<Self>,
        package: String,
        result: Result<CommandResult, String>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.install_in_progress = false;
        }

        let footer_message = match result {
            Ok(command) => {
                if command.success() {
                    let message = format!("\"{}\" installed successfully.", package);
                    self.show_toast(&format!("Installed {}.", package));
                    self.flag_installed_state(&package, true);
                    self.refresh_installed_packages();
                    Some(message)
                } else {
                    let mut detail = command.stderr.trim();
                    if detail.is_empty() {
                        detail = command.stdout.trim();
                    }
                    let message = if detail.is_empty() {
                        format!("Failed to install \"{}\".", package)
                    } else {
                        format!("Failed to install \"{}\": {}", package, detail)
                    };
                    self.show_error_dialog("Install Failed", &message);
                    Some(message)
                }
            }
            Err(err) => {
                let message = format!("Failed to install \"{}\": {}", package, err);
                self.show_error_dialog("Install Failed", &message);
                Some(message)
            }
        };
        self.update_discover_details();
        self.refresh_updates(true);
        self.rebuild_search_list();
        if let Some(msg) = footer_message {
            self.set_footer_message(Some(&msg));
        }
    }

    pub(crate) fn finish_remove(
        self: &Rc<Self>,
        package: String,
        result: Result<CommandResult, String>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.remove_in_progress = false;
            state.installed_selected.remove(&package);
            state.installed_detail_cache.remove(&package);
            state.installed_detail_loading.remove(&package);
            state.installed_detail_errors.remove(&package);
        }

        self.rebuild_installed_list();
        self.update_installed_selection_ui();

        let footer_message = match result {
            Ok(command) => {
                if command.success() {
                    let message = format!("\"{}\" removed successfully.", package);
                    self.set_installed_status_message(Some(message.clone()));
                    self.show_toast(&format!("Removed {}.", package));
                    self.flag_installed_state(&package, false);
                    self.refresh_installed_packages();
                    Some(message)
                } else {
                    let mut detail = command.stderr.trim();
                    if detail.is_empty() {
                        detail = command.stdout.trim();
                    }
                    let message = if detail.is_empty() {
                        format!("Failed to remove \"{}\".", package)
                    } else {
                        format!("Failed to remove \"{}\": {}", package, detail)
                    };
                    self.show_error_dialog("Removal Failed", &message);
                    Some(message)
                }
            }
            Err(err) => {
                let message = format!("Failed to remove \"{}\": {}", package, err);
                self.show_error_dialog("Removal Failed", &message);
                Some(message)
            }
        };
        self.update_discover_details();
        self.refresh_updates(true);
        self.rebuild_search_list();
        if let Some(msg) = footer_message {
            self.set_footer_message(Some(&msg));
        }
    }

    pub(crate) fn finish_remove_batch(
        self: &Rc<Self>,
        packages: Vec<String>,
        result: Result<CommandResult, String>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.remove_in_progress = false;
            for pkg in &packages {
                state.installed_selected.remove(pkg);
                state.installed_detail_cache.remove(pkg);
                state.installed_detail_loading.remove(pkg);
                state.installed_detail_errors.remove(pkg);
            }
        }

        self.rebuild_installed_list();
        self.update_installed_selection_ui();

        let footer_message = match result {
            Ok(command) => {
                if command.success() {
                    let message = if packages.len() == 1 {
                        format!("\"{}\" removed successfully.", packages[0])
                    } else {
                        "Selected packages removed successfully.".to_string()
                    };
                    self.set_installed_status_message(Some(message.clone()));
                    if packages.len() == 1 {
                        self.show_toast(&format!("Removed {}.", packages[0]));
                    } else {
                        self.show_toast("Selected packages removed.");
                    }
                    for pkg in packages {
                        self.flag_installed_state(&pkg, false);
                    }
                    self.refresh_installed_packages();
                    Some(message)
                } else {
                    let mut detail = command.stderr.trim();
                    if detail.is_empty() {
                        detail = command.stdout.trim();
                    }
                    let message = if detail.is_empty() {
                        "Failed to remove selected packages.".to_string()
                    } else {
                        format!("Failed to remove selected packages: {}", detail)
                    };
                    self.show_error_dialog("Removal Failed", &message);
                    Some(message)
                }
            }
            Err(err) => {
                let message = format!("Failed to remove selected packages: {}", err);
                self.show_error_dialog("Removal Failed", &message);
                Some(message)
            }
        };
        self.refresh_updates(true);
        if let Some(msg) = footer_message {
            self.set_footer_message(Some(&msg));
        }
    }

    pub(crate) fn show_toast(&self, message: &str) {
        let toast = adw::Toast::builder().title(message).timeout(5).build();
        self.widgets.toast_overlay.add_toast(toast);
    }

    pub(crate) fn show_error_dialog(&self, title: &str, message: &str) {
        let dialog = gtk::MessageDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .message_type(gtk::MessageType::Error)
            .text(title)
            .secondary_text(message)
            .build();
        dialog.add_button("Close", gtk::ResponseType::Close);
        dialog.connect_response(|dlg, _| dlg.close());
        dialog.present();
    }

    pub(crate) fn set_footer_message(&self, message: Option<&str>) {
        {
            let mut state = self.state.borrow_mut();
            state.footer_message = message.map(|m| m.to_string());
        }
        self.update_footer_text();
    }

    pub(crate) fn show_preferences(self: &Rc<Self>) {
        if let Some(existing) = self.preferences_window.borrow().as_ref() {
            existing.present();
            return;
        }

        let prefs = adw::PreferencesWindow::builder()
            .transient_for(&self.window)
            .modal(true)
            .title("Preferences")
            .build();
        prefs.set_application(Some(&self.app));
        self.preferences_window.replace(Some(prefs.clone()));

        {
            let controller = Rc::downgrade(self);
            prefs.connect_close_request(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.preferences_window.replace(None);
                }
                Propagation::Proceed
            });
        }
        {
            let controller = Rc::downgrade(self);
            prefs.connect_destroy(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.preferences_window.replace(None);
                }
            });
        }

        let general_page = adw::PreferencesPage::builder().title("General").build();

        let startup_group = adw::PreferencesGroup::builder()
            .title("Startup")
            .description("Choose what Nebula shows when it launches.")
            .build();
        let startup_model = gtk::StringList::new(&["Discover page", "Last viewed page"]);
        let start_combo = adw::ComboRow::builder()
            .title("Startup page")
            .model(&startup_model)
            .build();
        startup_group.add(&start_combo);
        general_page.add(&startup_group);

        let updates_group = adw::PreferencesGroup::builder()
            .title("Updates")
            .description("Control automatic update checks.")
            .build();
        let auto_switch_row = adw::ActionRow::builder()
            .title("Check for updates automatically")
            .build();
        let auto_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        auto_switch.set_active(self.state.borrow().auto_check_enabled);
        auto_switch_row.add_suffix(&auto_switch);
        auto_switch_row.set_activatable_widget(Some(&auto_switch));

        let frequency_model = gtk::StringList::new(&["Daily", "Weekly"]);
        let freq_combo = adw::ComboRow::builder()
            .title("Frequency")
            .model(&frequency_model)
            .build();
        freq_combo.set_sensitive(self.state.borrow().auto_check_enabled);

        updates_group.add(&auto_switch_row);
        updates_group.add(&freq_combo);

        let notify_switch_row = adw::ActionRow::builder()
            .title("Show system notifications for new updates")
            .build();
        let notify_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        notify_switch.set_active(self.state.borrow().notify_updates);
        notify_switch_row.add_suffix(&notify_switch);
        notify_switch_row.set_activatable_widget(Some(&notify_switch));
        updates_group.add(&notify_switch_row);
        general_page.add(&updates_group);

        let install_group = adw::PreferencesGroup::builder()
            .title("Install and Removal")
            .description("Ask for confirmation before changing packages.")
            .build();
        let confirm_install_row = adw::ActionRow::builder()
            .title("Confirm before installing packages")
            .build();
        let confirm_install_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        confirm_install_switch.set_active(self.state.borrow().confirm_install);
        confirm_install_row.add_suffix(&confirm_install_switch);
        confirm_install_row.set_activatable_widget(Some(&confirm_install_switch));

        let confirm_remove_row = adw::ActionRow::builder()
            .title("Confirm before removing packages")
            .build();
        let confirm_remove_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        confirm_remove_switch.set_active(self.state.borrow().confirm_remove);
        confirm_remove_row.add_suffix(&confirm_remove_switch);
        confirm_remove_row.set_activatable_widget(Some(&confirm_remove_switch));

        install_group.add(&confirm_install_row);
        install_group.add(&confirm_remove_row);
        general_page.add(&install_group);

        prefs.add(&general_page);

        {
            let start_combo_ref = start_combo.downgrade();
            let freq_combo_ref = freq_combo.downgrade();
            let initial_start = match self.state.borrow().start_page_preference {
                StartPagePreference::LastVisited => 1,
                StartPagePreference::Discover => 0,
            };
            let initial_freq = match self.state.borrow().auto_check_frequency {
                UpdateCheckFrequency::Daily => 0,
                UpdateCheckFrequency::Weekly => 1,
            };
            glib::idle_add_local(move || {
                if let Some(combo) = start_combo_ref.upgrade() {
                    combo.set_selected(initial_start);
                }
                if let Some(combo) = freq_combo_ref.upgrade() {
                    combo.set_selected(initial_freq);
                }
                glib::ControlFlow::Break
            });
        }

        let controller_clone = Rc::clone(self);
        start_combo.connect_selected_notify(move |row| {
            let preference = if row.selected() == 1 {
                StartPagePreference::LastVisited
            } else {
                StartPagePreference::Discover
            };
            controller_clone.update_start_page_preference(preference);
        });

        let controller_clone = Rc::clone(self);
        let freq_combo_clone = freq_combo.clone();
        auto_switch.connect_active_notify(move |switcher| {
            let active = switcher.is_active();
            controller_clone.set_auto_check_enabled(active, true);
            freq_combo_clone.set_sensitive(active);
        });

        let controller_clone = Rc::clone(self);
        freq_combo.connect_selected_notify(move |row| {
            let frequency = if row.selected() == 1 {
                UpdateCheckFrequency::Weekly
            } else {
                UpdateCheckFrequency::Daily
            };
            controller_clone.set_auto_check_frequency(frequency, true);
        });

        let controller_clone = Rc::clone(self);
        confirm_install_switch.connect_active_notify(move |switcher| {
            controller_clone.set_confirm_install(switcher.is_active(), true);
        });

        let controller_clone = Rc::clone(self);
        confirm_remove_switch.connect_active_notify(move |switcher| {
            controller_clone.set_confirm_remove(switcher.is_active(), true);
        });

        let controller_clone = Rc::clone(self);
        notify_switch.connect_active_notify(move |switcher| {
            controller_clone.set_notify_updates(switcher.is_active(), true);
        });

        prefs.present();
    }

    pub(crate) fn show_about_dialog(self: &Rc<Self>) {
        if let Some(existing) = self.about_dialog.borrow().as_ref() {
            existing.present();
            return;
        }

        let version = env!("CARGO_PKG_VERSION");
        let dialog = gtk::Dialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .title("About Nebula")
            .resizable(false)
            .build();
        dialog.set_application(Some(&self.app));

        let content = dialog.content_area();
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_spacing(12);

        let title = gtk::Label::builder()
            .label("Nebula")
            .halign(gtk::Align::Start)
            .build();
        title.add_css_class("title-3");

        let version_label = gtk::Label::builder()
            .label(&format!("Version {}", version))
            .halign(gtk::Align::Start)
            .build();
        version_label.add_css_class("dim-label");

        let description = gtk::Label::builder()
            .label("Nebula makes it easy to discover, install, and update software on Void Linux.")
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .halign(gtk::Align::Start)
            .build();
        description.set_xalign(0.0);

        let links_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .halign(gtk::Align::Start)
            .build();

        let make_link = |text: &str, url: &str| {
            let link = gtk::LinkButton::builder()
                .label(text)
                .uri(url)
                .halign(gtk::Align::Start)
                .build();
            link.add_css_class("flat");
            link
        };

        links_box.append(&make_link("Project website", "https://github.com/geektoshi/nebula"));
        links_box.append(&make_link(
            "Report an issue",
            "https://github.com/geektoshi/nebula/issues",
        ));
        links_box.append(&make_link(
            "Support & discussions",
            "https://github.com/geektoshi/nebula/discussions",
        ));

        content.append(&title);
        content.append(&version_label);
        content.append(&description);
        content.append(&links_box);

        dialog.add_button("Close", gtk::ResponseType::Close);
        dialog.connect_response(|dialog, _| dialog.close());

        {
            let controller = Rc::downgrade(self);
            dialog.connect_hide(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.about_dialog.replace(None);
                }
            });
        }

        {
            let controller = Rc::downgrade(self);
            dialog.connect_close_request(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.about_dialog.replace(None);
                }
                Propagation::Proceed
            });
        }

        self.about_dialog.replace(Some(dialog.clone()));
        dialog.present();
    }

    pub(crate) fn set_installed_status_message(&self, message: Option<String>) {
        {
            let mut state = self.state.borrow_mut();
            state.installed_status_message = message;
        }
        self.update_installed_summary();
    }

    pub(crate) fn cancel_auto_check_timer(&self) {
        if let Some(source) = self.state.borrow_mut().auto_check_source.take() {
            source.remove();
        }
    }

    pub(crate) fn clear_auto_check_handle(&self) {
        self.state.borrow_mut().auto_check_source = None;
    }

    pub(crate) fn schedule_auto_check(self: &Rc<Self>) {
        self.cancel_auto_check_timer();

        let (enabled, frequency) = {
            let state = self.state.borrow();
            (state.auto_check_enabled, state.auto_check_frequency)
        };

        if !enabled {
            return;
        }

        let interval = match frequency {
            UpdateCheckFrequency::Daily => 24 * 60 * 60,
            UpdateCheckFrequency::Weekly => 7 * 24 * 60 * 60,
        };

        let weak_self = Rc::downgrade(self);
        let source_id = glib::timeout_add_seconds_local(interval, move || {
            if let Some(controller) = weak_self.upgrade() {
                if !controller.state.borrow().auto_check_enabled {
                    controller.clear_auto_check_handle();
                    return glib::ControlFlow::Break;
                }

                controller.trigger_auto_check_from_timer();
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });

        self.state.borrow_mut().auto_check_source = Some(source_id);
    }

    pub(crate) fn trigger_auto_check_from_timer(self: &Rc<Self>) {
        if !self.can_trigger_auto_check_now() {
            return;
        }
        self.refresh_updates(true);
    }

    pub(crate) fn can_trigger_auto_check_now(&self) -> bool {
        let state = self.state.borrow();
        !state.updates_loading && !state.update_in_progress
    }

    pub(crate) fn flag_installed_state(self: &Rc<Self>, package_name: &str, installed: bool) {
        {
            let mut state = self.state.borrow_mut();
            if installed {
                state.installed_set.insert(package_name.to_string());
            } else {
                state.installed_set.remove(package_name);
            }

            for pkg in &mut state.search_results {
                if pkg.name == package_name {
                    pkg.installed = installed;
                }
            }

            if let Some(focus) = state.discover_detail_focus.as_mut() {
                if focus.name == package_name {
                    focus.installed = installed;
                }
            }

            if !installed {
                state
                    .installed_packages
                    .retain(|pkg| pkg.name != package_name);
            }
        }

        self.rebuild_search_list();
        self.rebuild_installed_list();
        self.update_spotlight_installed_flags();
        self.update_spotlight_views();
        self.refresh_active_spotlight_category();
        self.update_discover_details();
    }
}
