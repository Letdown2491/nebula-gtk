use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;
use gtk::glib::{self, Propagation};
use gtk::pango;

use crate::mirrors::{
    default_mirror_id, detect_active_repositories, find_mirror, humanize_base_url, map_urls_to_ids,
    set_active_mirrors_by_ids, tier1_mirrors, tor_mirrors, write_repository_config,
};
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
    pub(crate) update_buttons: RefCell<HashMap<String, gtk::Button>>,
    pub(crate) installed_action_boxes: RefCell<Vec<gtk::Widget>>,
    pub(crate) discover_buttons: RefCell<HashMap<String, gtk::Button>>,
    pub(crate) discover_row_stacks: RefCell<HashMap<String, gtk::Stack>>,
    pub(crate) discover_progress_bars: RefCell<HashMap<String, gtk::ProgressBar>>,
    pub(crate) preferences_window: RefCell<Option<adw::PreferencesWindow>>,
    pub(crate) mirrors_window: RefCell<Option<adw::PreferencesWindow>>,
    pub(crate) about_dialog: RefCell<Option<adw::MessageDialog>>,
    pub(crate) update_log_dialog: RefCell<Option<gtk::Dialog>>,
    pub(crate) update_log_buffer: RefCell<Option<gtk::TextBuffer>>,
    pub(crate) update_log_view: RefCell<Option<gtk::TextView>>,
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
        state.installed_row_buttons_visible = true;
        state.discover_row_buttons_visible = true;

        Self {
            widgets,
            sender,
            app,
            state: RefCell::new(state),
            window,
            settings,
            update_buttons: RefCell::new(HashMap::new()),
            installed_action_boxes: RefCell::new(Vec::new()),
            discover_buttons: RefCell::new(HashMap::new()),
            discover_row_stacks: RefCell::new(HashMap::new()),
            discover_progress_bars: RefCell::new(HashMap::new()),
            preferences_window: RefCell::new(None),
            mirrors_window: RefCell::new(None),
            about_dialog: RefCell::new(None),
            update_log_dialog: RefCell::new(None),
            update_log_buffer: RefCell::new(None),
            update_log_view: RefCell::new(None),
        }
    }

    pub(crate) fn setup_connections(self: &Rc<Self>) {
        self.widgets
            .discover
            .search_entry
            .connect_search_changed(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |entry| {
                    controller.on_discover_search_changed(entry.text().to_string());
                }
            ));

        self.widgets
            .installed
            .list_factory
            .connect_bind(glib::clone!(
                #[weak(rename_to = controller)]
                self,
                move |_, list_item| {
                    controller.bind_installed_list_item(list_item);
                }
            ));
        self.widgets
            .installed
            .list_factory
            .connect_unbind(glib::clone!(
                #[weak(rename_to = controller)]
                self,
                move |_, list_item| {
                    let actions_widget =
                        unsafe { list_item.steal_data::<gtk::Widget>("installed-actions") };
                    if let Some(actions_widget) = actions_widget {
                        controller
                            .installed_action_boxes
                            .borrow_mut()
                            .retain(|widget| widget != &actions_widget);
                    }
                    list_item.set_child(None::<&gtk::Widget>);
                }
            ));

        self.widgets
            .installed
            .list_selection
            .connect_selected_notify(glib::clone!(
                #[weak(rename_to = controller)]
                self,
                move |selection| {
                    let position = selection.selected();
                    if position == gtk::INVALID_LIST_POSITION {
                        controller.on_installed_row_selected(None);
                    } else {
                        controller.on_installed_row_selected(Some(position));
                    }
                }
            ));

        self.widgets
            .discover
            .search_entry
            .connect_activate(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_search_requested();
                }
            ));

        self.widgets
            .discover
            .list
            .connect_row_selected(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_, row| {
                    controller.on_search_row_selected(row.cloned());
                }
            ));
        self.widgets
            .discover
            .list
            .connect_row_activated(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_, _| {
                    controller.on_discover_primary_action();
                }
            ));
        self.widgets
            .discover
            .detail_action_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_discover_detail_action();
                }
            ));
        self.widgets
            .discover
            .detail_back_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_discover_detail_back();
                }
            ));
        self.widgets
            .discover
            .detail_close_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_discover_detail_close();
                }
            ));
        self.widgets
            .discover
            .spotlight_recent_list
            .connect_row_selected(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_, row| {
                    controller.on_spotlight_recent_selected(row.cloned());
                }
            ));
        self.widgets
            .discover
            .spotlight_recent_list
            .connect_row_activated(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_, row| {
                    controller.on_spotlight_row_activated(row);
                }
            ));
        self.widgets
            .discover
            .spotlight_recent_back_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_discover_detail_back();
                }
            ));
        self.widgets
            .discover
            .spotlight_recent_close_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_spotlight_recent_close();
                }
            ));
        self.widgets
            .discover
            .spotlight_recent_action_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_discover_primary_action();
                }
            ));
        self.widgets
            .discover
            .category_browsers_button
            .connect_toggled(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |btn| {
                    controller.handle_spotlight_category_toggle(
                        SpotlightCategory::Browsers,
                        btn.is_active(),
                    );
                }
            ));
        self.widgets
            .discover
            .category_chat_button
            .connect_toggled(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |btn| {
                    controller
                        .handle_spotlight_category_toggle(SpotlightCategory::Chat, btn.is_active());
                }
            ));
        self.widgets
            .discover
            .category_games_button
            .connect_toggled(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |btn| {
                    controller.handle_spotlight_category_toggle(
                        SpotlightCategory::Games,
                        btn.is_active(),
                    );
                }
            ));
        self.widgets
            .discover
            .category_graphics_button
            .connect_toggled(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |btn| {
                    controller.handle_spotlight_category_toggle(
                        SpotlightCategory::Graphics,
                        btn.is_active(),
                    );
                }
            ));
        self.widgets
            .discover
            .category_email_button
            .connect_toggled(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |btn| {
                    controller.handle_spotlight_category_toggle(
                        SpotlightCategory::Email,
                        btn.is_active(),
                    );
                }
            ));
        self.widgets
            .discover
            .category_music_button
            .connect_toggled(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |btn| {
                    controller.handle_spotlight_category_toggle(
                        SpotlightCategory::Music,
                        btn.is_active(),
                    );
                }
            ));
        self.widgets
            .discover
            .category_productivity_button
            .connect_toggled(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |btn| {
                    controller.handle_spotlight_category_toggle(
                        SpotlightCategory::Productivity,
                        btn.is_active(),
                    );
                }
            ));
        self.widgets
            .discover
            .category_utilities_button
            .connect_toggled(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |btn| {
                    controller.handle_spotlight_category_toggle(
                        SpotlightCategory::Utilities,
                        btn.is_active(),
                    );
                }
            ));
        self.widgets
            .discover
            .category_video_button
            .connect_toggled(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |btn| {
                    controller.handle_spotlight_category_toggle(
                        SpotlightCategory::Video,
                        btn.is_active(),
                    );
                }
            ));

        self.widgets
            .discover
            .spotlight_refresh_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.maybe_refresh_spotlight(true);
                }
            ));

        self.widgets
            .installed
            .refresh_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.refresh_installed_packages();
                }
            ));

        self.widgets
            .installed
            .search_entry
            .connect_search_changed(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |entry| {
                    controller.on_installed_search_changed(entry.text().to_string());
                }
            ));

        self.widgets
            .installed
            .search_entry
            .connect_activate(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |entry| {
                    controller.on_installed_search_changed(entry.text().to_string());
                }
            ));

        self.widgets
            .tools
            .cleanup_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_cleanup_requested();
                }
            ));
        {
            let spin_button = self.widgets.tools.cache_clean_spin_button.clone();
            self.widgets
                .tools
                .cache_clean_button
                .connect_clicked(glib::clone!(
                    #[strong(rename_to = controller)]
                    self,
                    move |_| {
                        let keep_n = spin_button.value() as u32;
                        controller.on_cache_clean_requested(keep_n);
                    }
                ));
        }
        self.widgets
            .tools
            .pkgdb_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_pkgdb_requested();
                }
            ));
        self.widgets
            .tools
            .reconfigure_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_reconfigure_requested();
                }
            ));
        self.widgets
            .tools
            .alternatives_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_alternatives_requested();
                }
            ));

        self.widgets
            .installed
            .filter_dropdown
            .connect_selected_notify(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |dropdown| {
                    controller.on_installed_filter_changed(dropdown.selected());
                }
            ));

        self.widgets
            .installed
            .remove_selected_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_installed_remove_selected();
                }
            ));

        self.widgets
            .installed
            .detail_back_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_installed_detail_back();
                }
            ));
        self.widgets
            .installed
            .detail_close_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_installed_detail_close();
                }
            ));

        self.widgets
            .installed
            .detail_remove_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_installed_detail_remove();
                }
            ));

        self.widgets
            .installed
            .detail_update_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_installed_detail_update();
                }
            ));

        self.widgets
            .installed
            .detail_pin_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_installed_detail_pin_toggle();
                }
            ));

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

        self.widgets
            .updates
            .check_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.refresh_updates(false);
                }
            ));
        self.widgets
            .updates
            .refresh_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.refresh_updates(false);
                }
            ));
        self.widgets
            .updates
            .logs_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.show_update_logs_dialog();
                }
            ));

        self.widgets
            .updates
            .update_all_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.update_all_packages();
                }
            ));

        self.widgets
            .updates
            .list
            .connect_row_activated(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_, row| {
                    controller.on_update_row_activated(row);
                }
            ));
        self.widgets
            .updates
            .detail_close_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_updates_detail_close();
                }
            ));
        self.widgets
            .updates
            .detail_update_button
            .connect_clicked(glib::clone!(
                #[strong(rename_to = controller)]
                self,
                move |_| {
                    controller.on_updates_detail_update();
                }
            ));

        self.widgets.updates.list.connect_row_selected(glib::clone!(
            #[strong(rename_to = controller)]
            self,
            move |_, row| {
                controller.on_update_row_selected(row.cloned());
            }
        ));

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
        self.switch_to_page(page);
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

    pub(crate) fn set_waypoint_before_upgrades(&self, enabled: bool, persist: bool) {
        if persist {
            {
                let mut settings = self.settings.borrow_mut();
                settings.waypoint_before_upgrades = enabled;
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
            state.installing_package = Some(package.name.clone());
        }

        self.rebuild_search_list();
        self.refresh_discover_install_widgets();
        self.restore_discover_focus_for(&package.name);

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
            state.removing_packages.insert(package.clone());
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
        self.refresh_discover_install_widgets();
        self.restore_discover_focus_for(&package);

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
            for pkg in &packages {
                state.removing_packages.insert(pkg.clone());
            }
        }

        self.update_installed_selection_ui();

        let message = format!(
            "Removing {} selected package{}…",
            packages.len(),
            if packages.len() == 1 { "" } else { "s" }
        );
        self.set_installed_status_message(Some(message.clone()));
        self.set_footer_message(Some(&message));

        self.refresh_discover_install_widgets();
        if let Some(focus) = self
            .state
            .borrow()
            .discover_detail_focus
            .as_ref()
            .map(|pkg| pkg.name.clone())
        {
            if packages.contains(&focus) {
                self.restore_discover_focus_for(&focus);
            }
        }

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
            Some("installed") => {
                if self.state.borrow().installed_packages.is_empty()
                    && !self.state.borrow().installed_refresh_in_progress
                {
                    self.refresh_installed_packages();
                }
            }
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
            AppMessage::PinOperationFinished {
                package,
                target_pinned,
                result,
            } => {
                self.finish_pin_toggle(package, target_pinned, result);
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
            AppMessage::UpdateLogLine { line } => {
                self.on_update_log_line(line);
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
            AppMessage::MirrorsDetected { mirrors } => {
                self.finish_mirror_detection(mirrors);
            }
            AppMessage::SnapshotComplete { result } => {
                self.finish_snapshot_creation(result);
            }
        }
    }

    fn finish_mirror_detection(self: &Rc<Self>, mirrors: Vec<String>) {
        if mirrors.is_empty() {
            return;
        }

        let filtered: Vec<String> = mirrors
            .into_iter()
            .filter(|id| find_mirror(id.as_str()).is_some())
            .collect();

        if filtered.is_empty() {
            return;
        }

        let current = self.state.borrow().selected_mirror_ids.clone();
        let current_set: std::collections::HashSet<_> = current.iter().collect();
        let filtered_set: std::collections::HashSet<_> = filtered.iter().collect();
        if current_set == filtered_set {
            return;
        }

        {
            let mut state = self.state.borrow_mut();
            state.selected_mirror_ids = filtered.clone();
        }

        set_active_mirrors_by_ids(&filtered);

        {
            let mut settings = self.settings.borrow_mut();
            settings.mirror_selection = filtered.clone();
            if let Err(err) = save_app_settings(&settings) {
                eprintln!("Failed to persist mirror selection: {}", err);
            }
        }

        self.show_toast("Detected active mirrors updated.");
        if let Some(window) = self.mirrors_window.borrow().as_ref() {
            window.queue_draw();
        }
    }

    fn finish_snapshot_creation(self: &Rc<Self>, result: crate::waypoint::SnapshotResult) {
        use crate::waypoint::SnapshotResult;

        // Check if we were waiting for a snapshot before update
        let pending_update = {
            let mut state = self.state.borrow_mut();
            state.footer_message.take()
        };

        let (package, from_all) = if let Some(pending) = pending_update {
            if pending.starts_with("snapshot_pending:") {
                let parts: Vec<&str> = pending.split(':').collect();
                if parts.len() == 3 {
                    (parts[1].to_string(), parts[2] == "true")
                } else {
                    return;
                }
            } else {
                return;
            }
        } else {
            return;
        };

        // Handle snapshot result
        match result {
            SnapshotResult::Success(snapshot_name) => {
                self.show_toast(&format!("Snapshot created: {}", snapshot_name));
                // Proceed with update
                self.execute_update(package, from_all);
            }
            SnapshotResult::Failure(error) => {
                // Show error toast with option to proceed anyway
                let toast = adw::Toast::builder()
                    .title(&format!("Snapshot failed: {}", error))
                    .button_label("Update Anyway")
                    .timeout(10)  // 10 seconds
                    .build();

                let controller = Rc::clone(self);
                let package_clone = package.clone();
                toast.connect_button_clicked(move |_| {
                    controller.execute_update(package_clone.clone(), from_all);
                });

                self.widgets.toast_overlay.add_toast(toast);
            }
            SnapshotResult::Timeout => {
                // Show timeout toast with option to proceed anyway
                let toast = adw::Toast::builder()
                    .title("Snapshot creation timed out")
                    .button_label("Update Anyway")
                    .timeout(10)  // 10 seconds
                    .build();

                let controller = Rc::clone(self);
                let package_clone = package.clone();
                toast.connect_button_clicked(move |_| {
                    controller.execute_update(package_clone.clone(), from_all);
                });

                self.widgets.toast_overlay.add_toast(toast);
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

    pub(crate) fn show_update_logs_dialog(self: &Rc<Self>) {
        if let Some(dialog) = self.update_log_dialog.borrow().as_ref() {
            self.refresh_update_log_buffer();
            dialog.present();
            return;
        }

        let dialog = gtk::Dialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .title("Update Logs")
            .build();
        dialog.set_default_size(640, 400);

        let content = dialog.content_area();
        content.set_spacing(12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);

        let scroller = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .build();
        let text_view = gtk::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .monospace(true)
            .wrap_mode(gtk::WrapMode::Char)
            .build();
        let buffer = text_view.buffer();
        self.populate_update_log_buffer(&buffer);
        self.update_log_buffer.replace(Some(buffer.clone()));
        scroller.set_child(Some(&text_view));
        content.append(&scroller);
        self.update_log_view.replace(Some(text_view.clone()));

        dialog.add_button("Close", gtk::ResponseType::Close);
        dialog.connect_response(|dialog, _| dialog.close());

        {
            let controller = Rc::downgrade(self);
            dialog.connect_hide(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.update_log_dialog.replace(None);
                    controller.update_log_buffer.replace(None);
                    controller.update_log_view.replace(None);
                }
            });
        }

        {
            let controller = Rc::downgrade(self);
            dialog.connect_close_request(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.update_log_dialog.replace(None);
                    controller.update_log_buffer.replace(None);
                    controller.update_log_view.replace(None);
                }
                Propagation::Proceed
            });
        }

        self.update_log_dialog.replace(Some(dialog.clone()));
        dialog.present();
    }

    pub(crate) fn refresh_update_log_buffer(&self) {
        if let Some(buffer) = self.update_log_buffer.borrow().as_ref() {
            self.populate_update_log_buffer(buffer);
        }
    }

    fn populate_update_log_buffer(&self, buffer: &gtk::TextBuffer) {
        let text = {
            let state = self.state.borrow();
            if state.update_log.is_empty() {
                "No update activity yet.".to_string()
            } else {
                state.update_log.join("\n")
            }
        };
        buffer.set_text(&text);
        let iter = buffer.end_iter();
        buffer.place_cursor(&iter);
        if let Some(view) = self.update_log_view.borrow().as_ref() {
            let mark = buffer.create_mark(None, &iter, false);
            view.scroll_to_mark(&mark, 0.0, true, 1.0, 1.0);
            buffer.delete_mark(&mark);
        }
    }

    pub(crate) fn append_update_log_buffer_line(&self, line: &str) {
        let is_first_line = {
            let state = self.state.borrow();
            state.update_log.len() == 1
        };

        if let Some(buffer) = self.update_log_buffer.borrow().as_ref() {
            if is_first_line {
                buffer.set_text(line);
            } else {
                let mut iter = buffer.end_iter();
                buffer.insert(&mut iter, "\n");
                buffer.insert(&mut iter, line);
            }

            let iter = buffer.end_iter();
            buffer.place_cursor(&iter);
            if let Some(view) = self.update_log_view.borrow().as_ref() {
                let mark = buffer.create_mark(None, &iter, false);
                view.scroll_to_mark(&mark, 0.0, true, 1.0, 1.0);
                buffer.delete_mark(&mark);
            }
        }
    }

    pub(crate) fn finish_install(
        self: &Rc<Self>,
        package: String,
        result: Result<CommandResult, String>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.install_in_progress = false;
            state.installing_package = None;
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
        self.refresh_discover_install_widgets();
        self.restore_discover_focus_for(&package);
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
        self.refresh_discover_install_widgets();
        self.restore_discover_focus_for(&package);
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
                state.removing_packages.remove(pkg);
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
                    for pkg in &packages {
                        self.flag_installed_state(pkg, false);
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
        self.rebuild_search_list();
        self.refresh_discover_install_widgets();
        if let Some(focus) = self
            .state
            .borrow()
            .discover_detail_focus
            .as_ref()
            .map(|pkg| pkg.name.clone())
        {
            if packages.iter().any(|pkg| *pkg == focus) {
                if !self.focus_discover_package(&focus, false) {
                    self.update_discover_details();
                }
            }
        } else {
            self.update_discover_details();
        }
        if let Some(msg) = footer_message {
            self.set_footer_message(Some(&msg));
        }
    }

    fn restore_discover_focus_for(self: &Rc<Self>, package: &str) {
        let should_restore = {
            let state = self.state.borrow();
            state
                .discover_detail_focus
                .as_ref()
                .map(|pkg| pkg.name == package)
                .unwrap_or(false)
        };

        if should_restore && !self.focus_discover_package(package, false) {
            self.update_discover_details();
        }
    }

    pub(crate) fn initialize_mirrors(self: &Rc<Self>) {
        let stored_ids = {
            let settings = self.settings.borrow();
            settings
                .mirror_selection
                .iter()
                .filter(|id| find_mirror(id.as_str()).is_some())
                .cloned()
                .collect::<Vec<_>>()
        };

        let mut initial_ids = if stored_ids.is_empty() {
            vec![default_mirror_id().to_string()]
        } else {
            stored_ids
        };

        initial_ids.retain(|id| find_mirror(id.as_str()).is_some());

        if initial_ids.is_empty() {
            initial_ids.push(default_mirror_id().to_string());
        }

        if let Err(err) = self.apply_mirror_selection(initial_ids.clone(), true, false) {
            eprintln!("Failed to initialize mirrors: {}", err);
        }

        let sender = self.sender.clone();
        thread::spawn(move || match detect_active_repositories() {
            Ok(urls) => {
                let mirrors = map_urls_to_ids(&urls);
                let _ = sender.send(AppMessage::MirrorsDetected { mirrors });
            }
            Err(err) => {
                eprintln!("Failed to detect active repositories: {}", err);
            }
        });
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

    pub(crate) fn show_mirrors(self: &Rc<Self>) {
        if let Some(window) = self.mirrors_window.borrow().as_ref() {
            window.present();
            return;
        }

        let window = adw::PreferencesWindow::builder()
            .transient_for(&self.window)
            .modal(true)
            .title("Mirrors")
            .build();
        window.set_application(Some(&self.app));
        self.mirrors_window.replace(Some(window.clone()));

        {
            let controller = Rc::downgrade(self);
            window.connect_close_request(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.mirrors_window.replace(None);
                }
                Propagation::Proceed
            });
        }

        {
            let controller = Rc::downgrade(self);
            window.connect_destroy(move |_| {
                if let Some(controller) = controller.upgrade() {
                    controller.mirrors_window.replace(None);
                }
            });
        }

        let page = adw::PreferencesPage::builder()
            .title("Repository Mirrors")
            .build();

        let tier_group = adw::PreferencesGroup::builder()
            .title("Tier 1 Mirrors")
            .description("Select the primary Void Linux mirrors Nebula should use.")
            .build();
        let tor_group = adw::PreferencesGroup::builder()
            .title("Tor Mirrors")
            .description("Requires the tor package. Follow https://docs.voidlinux.org/xbps/repositories/mirrors/tor.html before enabling these mirrors.")
            .build();

        {
            let selected = self.state.borrow().selected_mirror_ids.clone();
            let controller = Rc::downgrade(self);
            for mirror in tier1_mirrors() {
                let controller = controller.clone();
                let subtitle = humanize_base_url(mirror);
                let row = adw::ActionRow::builder()
                    .title(mirror.region)
                    .subtitle(&subtitle)
                    .activatable(true)
                    .build();
                let check = gtk::CheckButton::builder()
                    .valign(gtk::Align::Center)
                    .build();
                if selected.iter().any(|id| id == mirror.id) {
                    check.set_active(true);
                }
                let mirror_id = mirror.id.to_string();
                check.connect_toggled(move |btn| {
                    if let Some(controller) = controller.upgrade() {
                        controller.handle_mirror_toggle(&mirror_id, btn.is_active(), btn);
                    }
                });
                row.add_suffix(&check);
                row.set_activatable_widget(Some(&check));
                tier_group.add(&row);
            }
        }

        {
            let selected = self.state.borrow().selected_mirror_ids.clone();
            let controller = Rc::downgrade(self);
            for mirror in tor_mirrors() {
                let controller = controller.clone();
                let subtitle = humanize_base_url(mirror);
                let row = adw::ActionRow::builder()
                    .title(mirror.region)
                    .subtitle(&subtitle)
                    .activatable(true)
                    .build();
                let check = gtk::CheckButton::builder()
                    .valign(gtk::Align::Center)
                    .build();
                if selected.iter().any(|id| id == mirror.id) {
                    check.set_active(true);
                }
                let mirror_id = mirror.id.to_string();
                check.connect_toggled(move |btn| {
                    if let Some(controller) = controller.upgrade() {
                        controller.handle_mirror_toggle(&mirror_id, btn.is_active(), btn);
                    }
                });
                row.add_suffix(&check);
                row.set_activatable_widget(Some(&check));
                tor_group.add(&row);
            }
        }

        page.add(&tier_group);
        page.add(&tor_group);
        window.add(&page);
        window.present();
    }

    pub(crate) fn handle_mirror_toggle(
        self: &Rc<Self>,
        mirror_id: &str,
        active: bool,
        button: &gtk::CheckButton,
    ) {
        let mut selected = self.state.borrow().selected_mirror_ids.clone();
        let mut changed = false;

        if active {
            if !selected.iter().any(|id| id == mirror_id) {
                selected.push(mirror_id.to_string());
                changed = true;
            }
        } else {
            if selected.len() == 1 && selected.iter().any(|id| id == mirror_id) {
                self.restore_check_button(button, true);
                return;
            }
            let count_before = selected.len();
            selected.retain(|id| id != mirror_id);
            if selected.is_empty() {
                self.restore_check_button(button, true);
                return;
            }
            changed = count_before != selected.len();
        }

        if !changed {
            return;
        }

        match self.apply_mirror_selection(selected.clone(), true, true) {
            Ok(_) => {
                self.show_toast("Mirrors updated.");
                self.start_mirror_write_worker(selected.clone());
            }
            Err(err) => {
                self.show_error_dialog("Mirror Update Failed", &err);
                self.restore_check_button(button, !active);
            }
        }
    }

    fn restore_check_button(&self, button: &gtk::CheckButton, active: bool) {
        let weak = button.downgrade();
        glib::idle_add_local(move || {
            if let Some(button) = weak.upgrade() {
                button.set_active(active);
            }
            glib::ControlFlow::Break
        });
    }

    fn start_mirror_write_worker(self: &Rc<Self>, ids: Vec<String>) {
        let sender = self.sender.clone();
        thread::spawn(move || {
            if let Err(err) = write_repository_config(&ids) {
                eprintln!("Failed to write repository config: {}", err);
            }
            let _ = sender.send(AppMessage::MirrorsDetected { mirrors: ids });
        });
    }

    fn apply_mirror_selection(
        self: &Rc<Self>,
        ids: Vec<String>,
        persist_settings: bool,
        update_config: bool,
    ) -> Result<(), String> {
        if ids.is_empty() {
            return Err("At least one mirror must remain selected.".to_string());
        }

        if update_config {
            {
                let mut state = self.state.borrow_mut();
                state.selected_mirror_ids = ids.clone();
            }

            set_active_mirrors_by_ids(&ids);

            if persist_settings {
                let mut snapshot = {
                    let settings_ref = self.settings.borrow();
                    settings_ref.clone()
                };
                snapshot.mirror_selection = ids.clone();
                if let Err(err) = save_app_settings(&snapshot) {
                    eprintln!("Failed to save mirror selection: {}", err);
                }
                {
                    let mut settings = self.settings.borrow_mut();
                    *settings = snapshot;
                }
            } else {
                let mut settings = self.settings.borrow_mut();
                settings.mirror_selection = ids.clone();
            }

            self.start_mirror_write_worker(ids);
            Ok(())
        } else {
            {
                let mut state = self.state.borrow_mut();
                state.selected_mirror_ids = ids.clone();
            }
            set_active_mirrors_by_ids(&ids);

            if persist_settings {
                let mut snapshot = {
                    let settings_ref = self.settings.borrow();
                    settings_ref.clone()
                };
                snapshot.mirror_selection = ids.clone();
                if let Err(err) = save_app_settings(&snapshot) {
                    eprintln!("Failed to save mirror selection: {}", err);
                }
                {
                    let mut settings = self.settings.borrow_mut();
                    *settings = snapshot;
                }
            } else {
                let mut settings = self.settings.borrow_mut();
                settings.mirror_selection = ids.clone();
            }

            Ok(())
        }
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

        // Waypoint integration (only show if btrfs + waypoint available)
        let waypoint_switch_opt = if crate::waypoint::should_enable_integration() {
            let waypoint_switch_row = adw::ActionRow::builder()
                .title("Create snapshots before system updates")
                .subtitle("Requires Waypoint to create Btrfs snapshots")
                .build();
            let waypoint_switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
            waypoint_switch.set_active(self.settings.borrow().waypoint_before_upgrades);
            waypoint_switch_row.add_suffix(&waypoint_switch);
            waypoint_switch_row.set_activatable_widget(Some(&waypoint_switch));
            updates_group.add(&waypoint_switch_row);
            Some(waypoint_switch)
        } else {
            None
        };

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

        if let Some(waypoint_switch) = waypoint_switch_opt {
            let controller_clone = Rc::clone(self);
            waypoint_switch.connect_active_notify(move |switcher| {
                controller_clone.set_waypoint_before_upgrades(switcher.is_active(), true);
            });
        }

        prefs.present();
    }

    pub(crate) fn show_about_dialog(self: &Rc<Self>) {
        if let Some(existing) = self.about_dialog.borrow().as_ref() {
            existing.present();
            return;
        }

        let version = env!("CARGO_PKG_VERSION");
        let dialog = adw::MessageDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .build();

        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .halign(gtk::Align::Center)
            .build();
        content_box.set_margin_top(12);
        content_box.set_margin_bottom(12);
        content_box.set_margin_start(16);
        content_box.set_margin_end(16);

        let logo = gtk::Image::from_resource("/tech/geektoshi/Nebula/icons/nebula.png");
        logo.set_pixel_size(96);
        logo.set_valign(gtk::Align::Center);
        logo.set_halign(gtk::Align::Center);
        content_box.append(&logo);

        let title = gtk::Label::builder()
            .label("Nebula")
            .css_classes(["title-1"])
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .halign(gtk::Align::Center)
            .build();
        content_box.append(&title);

        let version_label = gtk::Label::builder()
            .label(&format!("Version {}", version))
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .css_classes(["dim-label"])
            .halign(gtk::Align::Center)
            .build();
        version_label.set_xalign(0.5);
        content_box.append(&version_label);

        let description = gtk::Label::builder()
            .label("Nebula is a GTK frontend for Void Linux's XBPS software tooling.")
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .css_classes(["dim-label"])
            .halign(gtk::Align::Center)
            .build();
        description.set_xalign(0.5);
        content_box.append(&description);

        let links_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .halign(gtk::Align::Center)
            .build();

        let make_link = |text: &str, url: &str| {
            let link = gtk::LinkButton::builder().label(text).uri(url).build();
            link.add_css_class("flat");
            link
        };

        links_row.append(&make_link(
            "Project website",
            "https://github.com/Letdown2491/nebula-gtk",
        ));

        let separator = gtk::Label::builder()
            .label("/")
            .halign(gtk::Align::Center)
            .build();
        separator.add_css_class("dim-label");
        links_row.append(&separator);

        links_row.append(&make_link(
            "Report an issue",
            "https://github.com/Letdown2491/nebula-gtk/issues",
        ));

        content_box.append(&links_row);

        dialog.set_extra_child(Some(&content_box));
        dialog.add_response("close", "Close");
        dialog.set_default_response(Some("close"));
        dialog.connect_response(None, |dialog, _| dialog.close());

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
