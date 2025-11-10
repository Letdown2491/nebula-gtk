mod detection;
mod snapshot;

pub use detection::{is_available, is_btrfs_root};
pub use snapshot::{create_pre_upgrade_snapshot, SnapshotResult};

/// Check if waypoint integration should be enabled
/// Returns true only if both btrfs is detected AND waypoint service is available
pub fn should_enable_integration() -> bool {
    is_btrfs_root() && is_available()
}
