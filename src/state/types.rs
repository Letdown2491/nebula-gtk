use std::collections::{HashMap, HashSet};

use gtk::glib;
use gtk4 as gtk;

use crate::details::{DiscoverDetail, InstalledDetail};
use crate::settings::{StartPagePreference, UpdateCheckFrequency};
use crate::spotlight::{SpotlightCache, SpotlightCategory};
use crate::state::controller::tools::{MaintenanceActionState, MaintenanceTask};
use crate::types::{CommandResult, PackageInfo};
use chrono::{DateTime, Utc};

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) search_results: Vec<PackageInfo>,
    pub(crate) installed_packages: Vec<PackageInfo>,
    pub(crate) installed_set: HashSet<String>,
    pub(crate) installed_filter: String,
    pub(crate) installed_filtered: Vec<usize>,
    pub(crate) installed_selected: HashSet<String>,
    pub(crate) installed_filter_mode: InstalledFilter,
    pub(crate) installed_last_refresh: Option<glib::DateTime>,
    pub(crate) selected_installed: Option<usize>,
    pub(crate) installed_detail_cache: HashMap<String, InstalledDetail>,
    pub(crate) installed_detail_loading: HashSet<String>,
    pub(crate) installed_detail_errors: HashMap<String, String>,
    pub(crate) installed_detail_package: Option<String>,
    pub(crate) installed_detail_history: Vec<String>,
    pub(crate) installed_detail_navigation_active: bool,
    pub(crate) installed_status_message: Option<String>,
    pub(crate) installed_row_buttons_visible: bool,
    pub(crate) available_updates: Vec<PackageInfo>,
    pub(crate) available_update_names: HashSet<String>,
    pub(crate) update_statuses: HashMap<String, UpdateStatus>,
    pub(crate) update_log: Vec<String>,
    pub(crate) updates_loading: bool,
    pub(crate) update_in_progress: bool,
    pub(crate) selected_updates: HashSet<String>,
    pub(crate) selected_update: Option<usize>,
    pub(crate) total_update_size: u64,
    pub(crate) last_update_check: Option<glib::DateTime>,
    pub(crate) auto_check_enabled: bool,
    pub(crate) auto_check_frequency: UpdateCheckFrequency,
    pub(crate) auto_check_source: Option<glib::SourceId>,
    pub(crate) selected_search: Option<usize>,
    pub(crate) search_in_progress: bool,
    pub(crate) install_in_progress: bool,
    pub(crate) remove_in_progress: bool,
    pub(crate) pin_in_progress: bool,
    pub(crate) installed_refresh_in_progress: bool,
    pub(crate) spotlight_cache: SpotlightCache,
    pub(crate) spotlight_recent: Vec<PackageInfo>,
    pub(crate) spotlight_categories: HashMap<SpotlightCategory, Vec<PackageInfo>>,
    pub(crate) spotlight_loading: bool,
    pub(crate) spotlight_last_refresh: Option<DateTime<Utc>>,
    pub(crate) active_spotlight_category: Option<SpotlightCategory>,
    pub(crate) spotlight_search_backup: Option<Vec<PackageInfo>>,
    pub(crate) spotlight_status_backup: Option<String>,
    pub(crate) spotlight_recent_selected: Option<String>,
    pub(crate) discover_mode: DiscoverMode,
    pub(crate) discover_detail_cache: HashMap<String, DiscoverDetail>,
    pub(crate) discover_detail_loading: HashSet<String>,
    pub(crate) discover_detail_errors: HashMap<String, String>,
    pub(crate) discover_detail_history: Vec<String>,
    pub(crate) discover_detail_navigation_active: bool,
    pub(crate) discover_detail_package: Option<String>,
    pub(crate) pending_discover_target: Option<String>,
    pub(crate) discover_detail_focus: Option<PackageInfo>,
    pub(crate) updates_detail_package: Option<String>,
    pub(crate) updates_detail_cache: HashMap<String, InstalledDetail>,
    pub(crate) updates_detail_loading: HashSet<String>,
    pub(crate) updates_detail_errors: HashMap<String, String>,
    pub(crate) start_page_preference: StartPagePreference,
    pub(crate) confirm_install: bool,
    pub(crate) confirm_remove: bool,
    pub(crate) footer_message: Option<String>,
    pub(crate) notify_updates: bool,
    pub(crate) updates_notification_sent: bool,
    pub(crate) maintenance_cleanup: MaintenanceActionState,
    pub(crate) maintenance_pkgdb: MaintenanceActionState,
    pub(crate) maintenance_reconfigure: MaintenanceActionState,
    pub(crate) maintenance_alternatives: MaintenanceActionState,
    pub(crate) selected_mirror_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UpdateStatus {
    Queued,
    Preparing,
    Downloading,
    Installing,
    Verifying,
    Completed,
    Failed,
}

impl UpdateStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            UpdateStatus::Queued => "Queued",
            UpdateStatus::Preparing => "Preparing",
            UpdateStatus::Downloading => "Downloading",
            UpdateStatus::Installing => "Installing",
            UpdateStatus::Verifying => "Verifying",
            UpdateStatus::Completed => "Completed",
            UpdateStatus::Failed => "Failed",
        }
    }

    pub(crate) fn precedence(self) -> u8 {
        match self {
            UpdateStatus::Queued => 0,
            UpdateStatus::Preparing => 1,
            UpdateStatus::Downloading => 2,
            UpdateStatus::Installing => 3,
            UpdateStatus::Verifying => 4,
            UpdateStatus::Completed => 5,
            UpdateStatus::Failed => 6,
        }
    }

    pub(crate) fn should_replace(self, current: UpdateStatus) -> bool {
        matches!(self, UpdateStatus::Failed) || self.precedence() >= current.precedence()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub(crate) enum DiscoverMode {
    #[default]
    Spotlight,
    Search,
}

#[allow(dead_code)]
pub(crate) enum AppMessage {
    SearchFinished {
        query: String,
        result: Result<Vec<PackageInfo>, String>,
    },
    InstalledFinished {
        result: Result<Vec<PackageInfo>, String>,
    },
    InstallFinished {
        package: String,
        result: Result<CommandResult, String>,
    },
    RemoveFinished {
        package: String,
        result: Result<CommandResult, String>,
    },
    RemoveBatchFinished {
        packages: Vec<String>,
        result: Result<CommandResult, String>,
    },
    PinOperationFinished {
        package: String,
        target_pinned: bool,
        result: Result<CommandResult, String>,
    },
    InstalledDetailsLoaded {
        package: String,
        result: Result<InstalledDetail, String>,
    },
    UpdatesDetailLoaded {
        package: String,
        result: Result<InstalledDetail, String>,
    },
    UpdatesRefreshed {
        packages: Vec<PackageInfo>,
        success: bool,
        error: Option<String>,
    },
    UpdateFinished {
        packages: Vec<String>,
        result: Result<CommandResult, String>,
        all: bool,
    },
    UpdateLogLine {
        line: String,
    },
    DiscoverDetailLoaded {
        package: String,
        result: Result<DiscoverDetail, String>,
    },
    SpotlightLoaded {
        recent: Vec<PackageInfo>,
        categories: HashMap<SpotlightCategory, Vec<PackageInfo>>,
        cache: SpotlightCache,
        refreshed_at: DateTime<Utc>,
    },
    SpotlightFailed {
        error: String,
    },
    MaintenanceFinished {
        task: MaintenanceTask,
        result: Result<CommandResult, String>,
    },
    MirrorsDetected {
        mirrors: Vec<String>,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum RemoveOrigin {
    Discover,
    Installed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub(crate) enum InstalledFilter {
    #[default]
    All,
    Updates,
}
