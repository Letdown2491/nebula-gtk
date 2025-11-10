use std::fs;
use zbus::blocking::Connection;

/// Check if the root filesystem is btrfs
pub fn is_btrfs_root() -> bool {
    // Read /proc/mounts and check if / is mounted as btrfs
    if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
        for line in mounts.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let mount_point = parts[1];
                let fs_type = parts[2];

                if mount_point == "/" && fs_type == "btrfs" {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if waypoint DBus service is available
pub fn is_available() -> bool {
    // Try to connect to system bus
    let Ok(connection) = Connection::system() else {
        return false;
    };

    // Check if tech.geektoshi.waypoint service is registered (running services)
    match connection.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "ListNames",
        &(),
    ) {
        Ok(reply) => {
            let body = reply.body();
            if let Ok(names) = body.deserialize::<Vec<String>>() {
                if names.iter().any(|name| name == "tech.geektoshi.waypoint") {
                    return true;
                }
            }
        }
        Err(_) => {}
    }

    // Also check activatable services (services that can be started on-demand)
    match connection.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "ListActivatableNames",
        &(),
    ) {
        Ok(reply) => {
            let body = reply.body();
            if let Ok(names) = body.deserialize::<Vec<String>>() {
                return names.iter().any(|name| name == "tech.geektoshi.waypoint");
            }
            false
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_btrfs_detection() {
        // This test will pass or fail based on actual system configuration
        let result = is_btrfs_root();
        println!("Btrfs root detected: {}", result);
    }

    #[test]
    fn test_waypoint_detection() {
        // This test will pass or fail based on whether waypoint is installed
        let result = is_available();
        println!("Waypoint available: {}", result);
    }
}
