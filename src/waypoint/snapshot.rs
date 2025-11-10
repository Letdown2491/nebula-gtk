use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use zbus::blocking::Connection;

/// Timeout for snapshot creation (30 seconds)
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(30);

/// DBus service details for waypoint
const WAYPOINT_SERVICE: &str = "tech.geektoshi.waypoint";
const WAYPOINT_PATH: &str = "/tech/geektoshi/waypoint";
const WAYPOINT_INTERFACE: &str = "tech.geektoshi.waypoint.Helper";

/// Result of snapshot creation
#[derive(Debug, Clone)]
pub enum SnapshotResult {
    Success(String),  // snapshot name
    Failure(String),  // error message
    Timeout,
}

/// Create a snapshot before system upgrade
/// Returns the snapshot name on success, or an error message on failure
pub fn create_pre_upgrade_snapshot(package_count: usize) -> SnapshotResult {
    let timestamp = chrono::Local::now().format("%y%m%d-%H%M");
    let name = format!("nebula-pre-upgrade-{}", timestamp);
    let description = format!(
        "Automatic snapshot by Nebula before upgrading {} package{}",
        package_count,
        if package_count == 1 { "" } else { "s" }
    );

    create_snapshot(&name, &description, vec!["/".to_string()])
}

/// Create a snapshot via DBus with the given name, description, and subvolumes
fn create_snapshot(name: &str, description: &str, subvolumes: Vec<String>) -> SnapshotResult {
    // Create a channel for timeout handling
    let (tx, rx) = mpsc::channel();
    let name_clone = name.to_string();
    let description_clone = description.to_string();

    // Spawn snapshot creation in separate thread
    thread::spawn(move || {
        let result = create_snapshot_impl(&name_clone, &description_clone, subvolumes);
        let _ = tx.send(result);
    });

    // Wait for result with timeout
    match rx.recv_timeout(SNAPSHOT_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            eprintln!("Waypoint snapshot creation timed out after {:?}", SNAPSHOT_TIMEOUT);
            SnapshotResult::Timeout
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            SnapshotResult::Failure("Internal error: channel disconnected".to_string())
        }
    }
}

/// Internal implementation of snapshot creation via DBus
fn create_snapshot_impl(name: &str, description: &str, subvolumes: Vec<String>) -> SnapshotResult {
    // Connect to system bus
    let connection = match Connection::system() {
        Ok(conn) => conn,
        Err(e) => {
            return SnapshotResult::Failure(format!("Failed to connect to system bus: {}", e));
        }
    };

    // Create proxy to waypoint service
    let proxy = match connection.call_method(
        Some(WAYPOINT_SERVICE),
        WAYPOINT_PATH,
        Some(WAYPOINT_INTERFACE),
        "CreateSnapshot",
        &(name, description, subvolumes),
    ) {
        Ok(reply) => reply,
        Err(e) => {
            return SnapshotResult::Failure(format!("Failed to call CreateSnapshot: {}", e));
        }
    };

    // Parse response (bool, String)
    let body = proxy.body();
    match body.deserialize::<(bool, String)>() {
        Ok((success, message)) => {
            if success {
                SnapshotResult::Success(name.to_string())
            } else {
                SnapshotResult::Failure(message)
            }
        }
        Err(e) => SnapshotResult::Failure(format!("Failed to parse response: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_naming() {
        let result = create_pre_upgrade_snapshot(42);
        match result {
            SnapshotResult::Success(name) => {
                assert!(name.starts_with("nebula-pre-upgrade-"));
                println!("Created snapshot: {}", name);
            }
            SnapshotResult::Failure(err) => {
                println!("Snapshot failed (expected if waypoint not installed): {}", err);
            }
            SnapshotResult::Timeout => {
                println!("Snapshot timed out");
            }
        }
    }
}
