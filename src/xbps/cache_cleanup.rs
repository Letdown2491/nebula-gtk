use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::types::CommandResult;

const XBPS_CACHE_DIR: &str = "/var/cache/xbps";

#[derive(Debug, Clone)]
struct CachedPackageFile {
    path: PathBuf,
    package_name: String,
    mtime: std::time::SystemTime,
    size: u64,
}

/// Check if xbps cache is currently locked by another process
pub(crate) fn is_cache_locked() -> bool {
    // Check if any xbps process is running
    if let Ok(output) = Command::new("pgrep")
        .arg("-x")
        .args(&["xbps-install", "xbps-remove", "xbps-pkgdb"])
        .output()
    {
        if output.status.success() && !output.stdout.is_empty() {
            return true;
        }
    }

    false
}

/// Extract package base name from cache filename
/// Format: packagename-version_revision.arch.xbps
/// Examples:
///   gtk4-devel-1.2.3_1.x86_64.xbps -> gtk4-devel
///   NetworkManager-1.50.0_1.x86_64.xbps -> NetworkManager
fn extract_package_name(filename: &str) -> Option<String> {
    // Remove .xbps extension
    let name = filename.strip_suffix(".xbps")?;

    // Remove architecture suffix (x86_64, i686, noarch, etc.)
    let name = if let Some(pos) = name.rfind('.') {
        &name[..pos]
    } else {
        name
    };

    // Find the version part (starts with a digit after a hyphen)
    // We go from right to left to find the last hyphen before a digit
    let mut last_hyphen_before_digit = None;
    let chars: Vec<char> = name.chars().collect();

    for i in (1..chars.len()).rev() {
        if chars[i].is_ascii_digit() && chars[i - 1] == '-' {
            last_hyphen_before_digit = Some(i - 1);
            break;
        }
    }

    if let Some(pos) = last_hyphen_before_digit {
        Some(name[..pos].to_string())
    } else {
        // Fallback: can't determine package name
        None
    }
}

/// List all cached package files with metadata
fn list_cached_files() -> Result<Vec<CachedPackageFile>, String> {
    let cache_path = PathBuf::from(XBPS_CACHE_DIR);

    if !cache_path.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&cache_path)
        .map_err(|e| format!("Failed to read cache directory: {}", e))?;

    let mut files = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        // Only process .xbps files
        if !path.extension().map_or(false, |ext| ext == "xbps") {
            continue;
        }

        let filename = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        // Extract package name
        let package_name = match extract_package_name(&filename) {
            Some(name) => name,
            None => continue,
        };

        // Get file metadata
        let metadata = match fs::metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };

        let mtime = metadata.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let size = metadata.len();

        files.push(CachedPackageFile {
            path,
            package_name,
            mtime,
            size,
        });
    }

    Ok(files)
}

/// Group cached files by package name and select files to remove
/// Keeps the N newest versions of each package (by mtime)
fn select_files_to_remove(files: Vec<CachedPackageFile>, keep_n: u32) -> Vec<CachedPackageFile> {
    let mut grouped: HashMap<String, Vec<CachedPackageFile>> = HashMap::new();

    // Group by package name
    for file in files {
        grouped.entry(file.package_name.clone())
            .or_insert_with(Vec::new)
            .push(file);
    }

    // For each package, sort by mtime and mark old ones for removal
    let mut to_remove = Vec::new();

    for (_package_name, mut versions) in grouped {
        if versions.len() <= keep_n as usize {
            // Keep all if we have fewer than or equal to keep_n versions
            continue;
        }

        // Sort by mtime, newest first
        versions.sort_by(|a, b| b.mtime.cmp(&a.mtime));

        // Keep the first keep_n, remove the rest
        to_remove.extend(versions.into_iter().skip(keep_n as usize));
    }

    to_remove
}

/// Remove cached package files using pkexec rm
fn remove_files(files: &[CachedPackageFile]) -> Result<CommandResult, String> {
    if files.is_empty() {
        return Ok(CommandResult {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    // Build list of file paths
    let file_paths: Vec<String> = files.iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();

    // Use pkexec to remove files
    // We'll call rm with multiple files at once, but need to be careful about
    // command line length limits. For now, let's batch them.
    const MAX_FILES_PER_CALL: usize = 100;

    let mut total_stdout = String::new();
    let mut total_stderr = String::new();

    for chunk in file_paths.chunks(MAX_FILES_PER_CALL) {
        let mut args = vec!["rm", "-f"];
        args.extend(chunk.iter().map(|s| s.as_str()));

        let output = Command::new("pkexec")
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to execute pkexec rm: {}", e))?;

        total_stdout.push_str(&String::from_utf8_lossy(&output.stdout));
        total_stderr.push_str(&String::from_utf8_lossy(&output.stderr));

        if !output.status.success() {
            return Err(format!(
                "Failed to remove cache files: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    Ok(CommandResult {
        code: Some(0),
        stdout: total_stdout,
        stderr: total_stderr,
    })
}

/// Clean package cache, keeping N latest versions of each package
/// If keep_n is 1, this behaves like `xbps-remove -o`
/// Returns the number of files removed and total size freed
pub(crate) fn clean_cache_keep_n(keep_n: u32) -> Result<(usize, u64), String> {
    // Check if cache is locked
    if is_cache_locked() {
        return Err("Package cache is currently in use by another xbps process. Please wait and try again.".to_string());
    }

    // List all cached files
    let files = list_cached_files()?;

    if files.is_empty() {
        return Ok((0, 0));
    }

    // Select files to remove
    let to_remove = select_files_to_remove(files, keep_n);

    if to_remove.is_empty() {
        return Ok((0, 0));
    }

    // Calculate total size to be freed
    let total_size: u64 = to_remove.iter().map(|f| f.size).sum();
    let file_count = to_remove.len();

    // Remove the files
    remove_files(&to_remove)?;

    Ok((file_count, total_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_package_name() {
        assert_eq!(
            extract_package_name("gtk4-devel-1.2.3_1.x86_64.xbps"),
            Some("gtk4-devel".to_string())
        );
        assert_eq!(
            extract_package_name("NetworkManager-1.50.0_1.x86_64.xbps"),
            Some("NetworkManager".to_string())
        );
        assert_eq!(
            extract_package_name("AppStream-1.0.4_2.x86_64.xbps"),
            Some("AppStream".to_string())
        );
        assert_eq!(
            extract_package_name("rust-1.75.0_1.x86_64.xbps"),
            Some("rust".to_string())
        );
        assert_eq!(
            extract_package_name("some-package-with-dashes-1.0.0_1.noarch.xbps"),
            Some("some-package-with-dashes".to_string())
        );
    }
}
