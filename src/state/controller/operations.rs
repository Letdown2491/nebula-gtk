use chrono::Utc;

use crate::state::controller::AppController;
use crate::state::types::{OperationStatus, OperationType, PackageOperation};
use crate::types::CommandResult;

impl AppController {
    /// Start tracking a new package operation
    pub(crate) fn start_operation_tracking(
        &self,
        package_name: String,
        operation_type: OperationType,
        command: String,
    ) {
        let operation = PackageOperation {
            package_name,
            operation_type,
            status: OperationStatus::InProgress,
            started_at: Utc::now(),
            completed_at: None,
            command,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            size_bytes: None,
            error_message: None,
        };

        let mut state = self.state.borrow_mut();
        state.operation_history.push(operation);

        // Keep only the most recent N operations (default 50)
        let max = if state.max_operation_history == 0 {
            50
        } else {
            state.max_operation_history
        };

        if state.operation_history.len() > max {
            let current_len = state.operation_history.len();
            state.operation_history.drain(0..current_len - max);
        }
    }

    /// Complete an operation and update its status
    pub(crate) fn complete_operation_tracking(
        &self,
        package_name: &str,
        result: &Result<CommandResult, String>,
    ) {
        let mut state = self.state.borrow_mut();

        // Find the most recent in-progress operation for this package
        if let Some(operation) = state
            .operation_history
            .iter_mut()
            .rev()
            .find(|op| op.package_name == package_name && op.status == OperationStatus::InProgress)
        {
            operation.completed_at = Some(Utc::now());

            match result {
                Ok(cmd_result) => {
                    operation.exit_code = cmd_result.code;
                    operation.stdout = cmd_result.stdout.clone();
                    operation.stderr = cmd_result.stderr.clone();

                    if cmd_result.success() {
                        operation.status = OperationStatus::Success;
                    } else {
                        operation.status = OperationStatus::Failed;
                        operation.error_message = Some(
                            if !cmd_result.stderr.trim().is_empty() {
                                cmd_result.stderr.trim().to_string()
                            } else if !cmd_result.stdout.trim().is_empty() {
                                cmd_result.stdout.trim().to_string()
                            } else {
                                format!("Exit code: {}", cmd_result.code.unwrap_or(-1))
                            }
                        );
                    }
                }
                Err(err) => {
                    operation.status = OperationStatus::Failed;
                    operation.error_message = Some(err.clone());
                    operation.stderr = err.clone();
                }
            }
        }
    }

    /// Get the most recent operation for a specific package
    #[allow(dead_code)]
    pub(crate) fn get_recent_operation(&self, package_name: &str) -> Option<PackageOperation> {
        let state = self.state.borrow();
        state
            .operation_history
            .iter()
            .rev()
            .find(|op| op.package_name == package_name)
            .cloned()
    }

    /// Get all operations, most recent first
    pub(crate) fn get_all_operations(&self) -> Vec<PackageOperation> {
        let state = self.state.borrow();
        let mut ops = state.operation_history.clone();
        ops.reverse(); // Most recent first
        ops
    }

    /// Clear operation history
    pub(crate) fn clear_operation_history(&self) {
        let mut state = self.state.borrow_mut();
        state.operation_history.clear();
    }

    /// Create a status indicator widget for a package's recent operation
    pub(crate) fn create_operation_status_indicator(&self, package_name: &str) -> Option<gtk4::Widget> {
        use gtk4 as gtk;
        use gtk4::prelude::*;

        let operation = self.get_recent_operation(package_name)?;

        // Only show indicators for recently completed operations (within last 5 minutes)
        if let Some(completed_at) = operation.completed_at {
            let now = chrono::Utc::now();
            let duration = now.signed_duration_since(completed_at);
            if duration.num_minutes() > 5 {
                return None;
            }
        }

        let (icon_name, css_class, tooltip) = match operation.status {
            OperationStatus::Success => (
                "object-select-symbolic",
                "success",
                match operation.operation_type {
                    OperationType::Install => format!("Successfully installed"),
                    OperationType::Remove => format!("Successfully removed"),
                    #[allow(dead_code)]
                    OperationType::Update { .. } => format!("Successfully updated"),
                }
            ),
            OperationStatus::Failed => (
                "process-stop-symbolic",
                "error",
                format!("Operation failed: {}", operation.error_message.as_deref().unwrap_or("Unknown error"))
            ),
            OperationStatus::InProgress => (
                "view-refresh-symbolic",
                "accent",
                format!("Operation in progress...")
            ),
            #[allow(dead_code)]
            OperationStatus::Warning => (
                "dialog-warning-symbolic",
                "warning",
                format!("Operation completed with warnings")
            ),
        };

        let icon = gtk::Image::builder()
            .icon_name(icon_name)
            .pixel_size(16)
            .valign(gtk::Align::Center)
            .tooltip_text(&tooltip)
            .build();
        icon.add_css_class(css_class);

        Some(icon.upcast())
    }
}
