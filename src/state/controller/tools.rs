use std::rc::Rc;
use std::thread;

use gtk4 as gtk;
use libadwaita as adw;

use adw::prelude::*;

use chrono::{DateTime, Utc};

use crate::state::controller::AppController;
use crate::state::types::AppMessage;
use crate::types::CommandResult;
use crate::xbps::{
    run_xbps_alternatives_list, run_xbps_pkgdb_check, run_xbps_reconfigure_all,
    run_xbps_remove_cache, run_xbps_remove_orphans, summarize_output_line,
};

impl AppController {
    pub(crate) fn on_cleanup_requested(self: &Rc<Self>) {
        self.start_maintenance_task(MaintenanceTask::Cleanup);
    }

    pub(crate) fn on_pkgdb_requested(self: &Rc<Self>) {
        self.start_maintenance_task(MaintenanceTask::Pkgdb);
    }

    pub(crate) fn on_reconfigure_requested(self: &Rc<Self>) {
        self.start_maintenance_task(MaintenanceTask::Reconfigure);
    }

    pub(crate) fn on_alternatives_requested(self: &Rc<Self>) {
        self.start_maintenance_task(MaintenanceTask::Alternatives);
    }

    pub(crate) fn on_cache_clean_requested(self: &Rc<Self>, keep_n: u32) {
        if keep_n == 1 {
            // Use fast path with xbps-remove -o
            self.start_maintenance_task(MaintenanceTask::CacheClean);
        } else {
            // Use custom cleanup logic
            self.start_cache_clean_keep_n(keep_n);
        }
    }

    fn start_cache_clean_keep_n(self: &Rc<Self>, keep_n: u32) {
        {
            let mut state = self.state.borrow_mut();
            let action_state = &mut state.maintenance_cache_clean;

            if action_state.running {
                return;
            }

            action_state.running = true;
            action_state.last_success = None;
            action_state.last_message = None;
            action_state.last_stdout = None;
            action_state.last_stderr = None;
            action_state.last_finished_at = None;
        }

        self.update_tools_actions();

        let sender = self.sender.clone();
        thread::spawn(move || {
            use crate::xbps::clean_cache_keep_n;
            use crate::xbps::format_size;

            let result = match clean_cache_keep_n(keep_n) {
                Ok((count, size)) => {
                    let size_str = format_size(size);
                    Ok(CommandResult {
                        code: Some(0),
                        stdout: format!("Removed {} package(s), freed {}", count, size_str),
                        stderr: String::new(),
                    })
                }
                Err(e) => Err(e),
            };

            let _ = sender.send(AppMessage::MaintenanceFinished {
                task: MaintenanceTask::CacheClean,
                result,
            });
        });
    }

    pub(crate) fn start_maintenance_task(self: &Rc<Self>, task: MaintenanceTask) {
        {
            let mut state = self.state.borrow_mut();
            let action_state = match task {
                MaintenanceTask::Cleanup => &mut state.maintenance_cleanup,
                MaintenanceTask::Pkgdb => &mut state.maintenance_pkgdb,
                MaintenanceTask::Reconfigure => &mut state.maintenance_reconfigure,
                MaintenanceTask::Alternatives => &mut state.maintenance_alternatives,
                MaintenanceTask::CacheClean => &mut state.maintenance_cache_clean,
            };

            if action_state.running {
                return;
            }

            action_state.running = true;
            action_state.last_success = None;
            action_state.last_message = None;
            action_state.last_stdout = None;
            action_state.last_stderr = None;
            action_state.last_finished_at = None;
        }

        self.update_tools_actions();

        let sender = self.sender.clone();
        thread::spawn(move || {
            let result = match task {
                MaintenanceTask::Cleanup => run_xbps_remove_orphans(),
                MaintenanceTask::Pkgdb => run_xbps_pkgdb_check(),
                MaintenanceTask::Reconfigure => run_xbps_reconfigure_all(),
                MaintenanceTask::Alternatives => run_xbps_alternatives_list(),
                MaintenanceTask::CacheClean => run_xbps_remove_cache(),
            };
            let _ = sender.send(AppMessage::MaintenanceFinished { task, result });
        });
    }

    pub(crate) fn finish_maintenance(
        self: &Rc<Self>,
        task: MaintenanceTask,
        result: Result<CommandResult, String>,
    ) {
        let finished_at = Utc::now();
        let copy = maintenance_copy(task);

        let (success, status_message, toast_message, stdout_store, stderr_store) = match result {
            Ok(cmd_result) => {
                let stdout_summary = summarize_output_line(&cmd_result.stdout);
                let stderr_summary = summarize_output_line(&cmd_result.stderr);

                if cmd_result.success() {
                    let mut status_message = copy.success_message.to_string();
                    if let Some(line) = stdout_summary.clone() {
                        status_message.push(' ');
                        status_message.push_str(&line);
                    }
                    let toast_message = copy.success_toast.to_string();
                    (
                        true,
                        status_message,
                        toast_message,
                        Some(cmd_result.stdout.clone()),
                        Some(cmd_result.stderr.clone()),
                    )
                } else {
                    let detail = stderr_summary.clone().or(stdout_summary.clone());
                    let status_message = if let Some(line) = detail {
                        format!("{}: {}", copy.failure_prefix, line)
                    } else if let Some(code) = cmd_result.code {
                        format!("{} (exit code {}).", copy.failure_prefix, code)
                    } else {
                        format!("{}.", copy.failure_prefix)
                    };
                    let toast_message = copy.failure_toast.to_string();
                    (
                        false,
                        status_message,
                        toast_message,
                        Some(cmd_result.stdout.clone()),
                        Some(cmd_result.stderr.clone()),
                    )
                }
            }
            Err(err) => {
                let status_message = format!("{}: {}", copy.failure_prefix, err);
                let toast_message = copy.failure_toast.to_string();
                (false, status_message, toast_message, None, Some(err))
            }
        };

        {
            let mut state = self.state.borrow_mut();
            let action_state = match task {
                MaintenanceTask::Cleanup => &mut state.maintenance_cleanup,
                MaintenanceTask::CacheClean => &mut state.maintenance_cache_clean,
                MaintenanceTask::Pkgdb => &mut state.maintenance_pkgdb,
                MaintenanceTask::Reconfigure => &mut state.maintenance_reconfigure,
                MaintenanceTask::Alternatives => &mut state.maintenance_alternatives,
            };
            action_state.running = false;
            action_state.last_success = Some(success);
            action_state.last_message = Some(status_message.clone());
            action_state.last_stdout = stdout_store.clone();
            action_state.last_stderr = stderr_store.clone();
            action_state.last_finished_at = Some(finished_at);

            // Update footer status
            state.tools_status_message = Some(status_message.clone());
            state.tools_status_is_error = !success;
        }

        self.update_tools_actions();

        if success && matches!(task, MaintenanceTask::Alternatives) {
            if let Some(stdout) = stdout_store {
                self.show_alternatives_dialog(&stdout);
            }
        }

        self.show_toast(&toast_message);
    }

    pub(crate) fn update_tools_actions(&self) {
        let state = self.state.borrow();
        self.update_maintenance_row(
            MaintenanceTask::Cleanup,
            &state.maintenance_cleanup,
            &self.widgets.tools.cleanup_button,
            &self.widgets.tools.cleanup_spinner,
        );
        self.update_maintenance_row(
            MaintenanceTask::CacheClean,
            &state.maintenance_cache_clean,
            &self.widgets.tools.cache_clean_button,
            &self.widgets.tools.cache_clean_spinner,
        );
        self.update_maintenance_row(
            MaintenanceTask::Pkgdb,
            &state.maintenance_pkgdb,
            &self.widgets.tools.pkgdb_button,
            &self.widgets.tools.pkgdb_spinner,
        );
        self.update_maintenance_row(
            MaintenanceTask::Reconfigure,
            &state.maintenance_reconfigure,
            &self.widgets.tools.reconfigure_button,
            &self.widgets.tools.reconfigure_spinner,
        );
        self.update_maintenance_row(
            MaintenanceTask::Alternatives,
            &state.maintenance_alternatives,
            &self.widgets.tools.alternatives_button,
            &self.widgets.tools.alternatives_spinner,
        );
        drop(state);
        self.update_tools_status_footer();
    }

    fn update_maintenance_row(
        &self,
        _task: MaintenanceTask,
        state: &MaintenanceActionState,
        button: &gtk::Button,
        spinner: &gtk::Spinner,
    ) {
        if state.running {
            button.set_sensitive(false);
            spinner.set_visible(true);
            spinner.start();
            return;
        }

        spinner.stop();
        spinner.set_visible(false);
        button.set_sensitive(true);
    }

    fn update_tools_status_footer(&self) {
        let state = self.state.borrow();

        // Check if any task is currently running
        let running_task = if state.maintenance_cleanup.running {
            Some((MaintenanceTask::Cleanup, &state.maintenance_cleanup))
        } else if state.maintenance_cache_clean.running {
            Some((MaintenanceTask::CacheClean, &state.maintenance_cache_clean))
        } else if state.maintenance_pkgdb.running {
            Some((MaintenanceTask::Pkgdb, &state.maintenance_pkgdb))
        } else if state.maintenance_reconfigure.running {
            Some((MaintenanceTask::Reconfigure, &state.maintenance_reconfigure))
        } else if state.maintenance_alternatives.running {
            Some((MaintenanceTask::Alternatives, &state.maintenance_alternatives))
        } else {
            None
        };

        if let Some((task, _)) = running_task {
            let copy = maintenance_copy(task);
            self.widgets.tools.status_label.set_text(copy.running_text);
            self.widgets.tools.status_label.remove_css_class("success");
            self.widgets.tools.status_label.remove_css_class("error");
            self.widgets.tools.status_revealer.set_reveal_child(true);
        } else if let Some(ref message) = state.tools_status_message {
            self.widgets.tools.status_label.set_text(message);
            self.widgets.tools.status_label.remove_css_class("success");
            self.widgets.tools.status_label.remove_css_class("error");
            if state.tools_status_is_error {
                self.widgets.tools.status_label.add_css_class("error");
            } else {
                self.widgets.tools.status_label.add_css_class("success");
            }
            self.widgets.tools.status_revealer.set_reveal_child(true);
        } else {
            self.widgets.tools.status_revealer.set_reveal_child(false);
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum MaintenanceTask {
    Cleanup,
    Pkgdb,
    Reconfigure,
    Alternatives,
    CacheClean,
}

#[derive(Default)]
pub(crate) struct MaintenanceActionState {
    pub(crate) running: bool,
    pub(crate) last_success: Option<bool>,
    pub(crate) last_message: Option<String>,
    pub(crate) last_stdout: Option<String>,
    pub(crate) last_stderr: Option<String>,
    pub(crate) last_finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy)]
pub(crate) struct MaintenanceCopy {
    #[allow(dead_code)]
    pub(crate) idle_text: &'static str,
    pub(crate) running_text: &'static str,
    pub(crate) success_message: &'static str,
    pub(crate) failure_prefix: &'static str,
    pub(crate) success_toast: &'static str,
    pub(crate) failure_toast: &'static str,
}

pub(crate) fn maintenance_copy(task: MaintenanceTask) -> MaintenanceCopy {
    match task {
        MaintenanceTask::Cleanup => MaintenanceCopy {
            idle_text: "Haven't run this cleanup yet.",
            running_text: "Tidying unused packages...",
            success_message: "Cleanup finished without finding any stragglers.",
            failure_prefix: "Cleanup ran into an issue",
            success_toast: "Cleanup complete.",
            failure_toast: "Cleanup failed.",
        },
        MaintenanceTask::Pkgdb => MaintenanceCopy {
            idle_text: "No database check yet.",
            running_text: "Reviewing the package database...",
            success_message: "Package database check came back clean.",
            failure_prefix: "Package database check hit a snag",
            success_toast: "Package database check complete.",
            failure_toast: "Package database check failed.",
        },
        MaintenanceTask::Reconfigure => MaintenanceCopy {
            idle_text: "Haven't reconfigured anything this session.",
            running_text: "Re-running every package's setup...",
            success_message: "Reconfigure finished. Services should be up to date.",
            failure_prefix: "Reconfigure didn't finish",
            success_toast: "Reconfigure complete.",
            failure_toast: "Reconfigure failed.",
        },
        MaintenanceTask::Alternatives => MaintenanceCopy {
            idle_text: "Haven't opened the alternatives list yet.",
            running_text: "Collecting the alternatives list...",
            success_message: "Alternatives list loaded.",
            failure_prefix: "Couldn't load alternatives",
            success_toast: "Alternatives list ready.",
            failure_toast: "Failed to load alternatives.",
        },
        MaintenanceTask::CacheClean => MaintenanceCopy {
            idle_text: "Ready to clean cache.",
            running_text: "Cleaning obsolete cached packages...",
            success_message: "Cache cleaned successfully.",
            failure_prefix: "Cache cleaning encountered an issue",
            success_toast: "Package cache cleaned.",
            failure_toast: "Cache cleaning failed.",
        },
    }
}
