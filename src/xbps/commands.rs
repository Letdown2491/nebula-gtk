use std::collections::{HashMap, HashSet};
use std::process::Command;

use crate::types::{CommandResult, DependencyInfo, PackageInfo, lowercase_cache};

use super::parser::{
    parse_bytes, parse_bytes_from_field, parse_installed_output, parse_long_description,
    parse_query_output, split_package_identifier, strip_ansi_codes,
};
use super::privilege::run_privileged_command;

pub(crate) fn run_xbps_query_dependencies(package: &str) -> Result<Vec<DependencyInfo>, String> {
    let output = Command::new("xbps-query")
        .args(["-R", "--show", package])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut dependencies = Vec::new();
    let mut in_run_depends = false;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(spec) = trimmed.strip_prefix("run_depends:") {
            in_run_depends = true;
            let spec = spec.trim().trim_matches(|c| c == '\'' || c == '"');
            if !spec.is_empty() {
                let name_part = spec
                    .split(|c: char| matches!(c, '<' | '>' | '=' | ' '))
                    .next()
                    .unwrap_or(spec)
                    .trim()
                    .trim_end_matches('?');
                if !name_part.is_empty() {
                    dependencies.push(DependencyInfo {
                        name: name_part.to_string(),
                    });
                }
            }
            continue;
        }

        if in_run_depends {
            if trimmed.is_empty() {
                in_run_depends = false;
                continue;
            }

            let first_char = line.chars().next().unwrap_or_default();
            if !first_char.is_whitespace() {
                in_run_depends = false;
                continue;
            }

            if trimmed.contains(':') {
                in_run_depends = false;
                continue;
            }

            let spec = trimmed.trim_matches(|c| c == '\'' || c == '"');
            if spec.is_empty() {
                continue;
            }
            let name_part = spec
                .split(|c: char| matches!(c, '<' | '>' | '=' | ' '))
                .next()
                .unwrap_or(spec)
                .trim()
                .trim_end_matches('?');
            if name_part.is_empty() {
                continue;
            }
            dependencies.push(DependencyInfo {
                name: name_part.to_string(),
            });
        }
    }

    dependencies.sort_by(|a, b| a.name.cmp(&b.name));
    dependencies.dedup_by(|a, b| a.name == b.name);

    Ok(dependencies)
}

pub(crate) fn run_xbps_query_search(query: &str) -> Result<Vec<PackageInfo>, String> {
    let output = Command::new("xbps-query")
        .args(["-R", "--regex", "-s", query])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_query_output(&stdout))
}

pub(crate) fn run_xbps_list_installed() -> Result<Vec<PackageInfo>, String> {
    let output = Command::new("xbps-query")
        .arg("-l")
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_installed_output(&stdout))
}

pub(crate) fn run_xbps_install(package: &str) -> Result<CommandResult, String> {
    run_privileged_command("xbps-install", &["-y", package])
}

pub(crate) fn run_xbps_remove(package: &str) -> Result<CommandResult, String> {
    run_xbps_remove_packages(&[package.to_string()])
}

pub(crate) fn run_xbps_remove_packages(packages: &[String]) -> Result<CommandResult, String> {
    if packages.is_empty() {
        return Ok(CommandResult {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    let mut args = vec!["-y"];
    let package_refs: Vec<&str> = packages.iter().map(|pkg| pkg.as_str()).collect();
    args.extend(package_refs);
    run_privileged_command("xbps-remove", &args)
}

pub(crate) fn run_xbps_query_required_by(package: &str) -> Result<Vec<String>, String> {
    let output = Command::new("xbps-query")
        .args(["-X", package])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut required = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (name, _) = split_package_identifier(trimmed);
        if !name.is_empty() {
            required.push(name);
        }
    }
    required.sort();
    required.dedup();
    Ok(required)
}

pub(crate) fn query_pkgsize_bytes(package: &str) -> Result<Option<u64>, String> {
    if let Some(bytes) = query_size_property(package, "installed_size")? {
        return Ok(Some(bytes));
    }
    query_size_property(package, "pkgsize")
}

fn query_size_property(package: &str, property: &str) -> Result<Option<u64>, String> {
    let output = Command::new("xbps-query")
        .args(["-p", property, package])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        let value = trimmed
            .strip_prefix(property)
            .and_then(|s| s.strip_prefix(':'))
            .map(|v| v.trim())
            .unwrap_or(trimmed);
        if let Some(bytes) = parse_bytes_from_field(value) {
            return Ok(Some(bytes));
        }
    }

    Ok(None)
}

#[derive(Default, Debug)]
pub(crate) struct PackageMetadata {
    pub long_desc: Option<String>,
    pub homepage: Option<String>,
    pub maintainer: Option<String>,
    pub license: Option<String>,
    pub repository: Option<String>,
}

pub(crate) fn query_package_metadata(package: &str) -> PackageMetadata {
    const PROPERTIES: [&str; 5] = [
        "long_desc",
        "homepage",
        "maintainer",
        "license",
        "repository",
    ];
    let mut metadata = PackageMetadata::default();

    if let Some(values) = query_properties_from_show(package, &PROPERTIES, false) {
        apply_package_metadata(&values, &mut metadata);
    }

    if metadata.long_desc.is_none()
        || metadata.homepage.is_none()
        || metadata.maintainer.is_none()
        || metadata.license.is_none()
        || metadata.repository.is_none()
    {
        if let Some(values) = query_properties_from_show(package, &PROPERTIES, true) {
            apply_package_metadata(&values, &mut metadata);
        }
    }

    metadata
}

fn apply_package_metadata(
    values: &HashMap<String, String>,
    metadata: &mut PackageMetadata,
) {
    if metadata.long_desc.is_none() {
        if let Some(long_desc) = values.get("long_desc").and_then(parse_long_description) {
            metadata.long_desc = Some(long_desc);
        }
    }
    if metadata.homepage.is_none() {
        if let Some(homepage) = values.get("homepage").and_then(clean_simple_property) {
            metadata.homepage = Some(homepage);
        }
    }
    if metadata.maintainer.is_none() {
        if let Some(maintainer) = values.get("maintainer").and_then(clean_simple_property) {
            metadata.maintainer = Some(maintainer);
        }
    }
    if metadata.license.is_none() {
        if let Some(license) = values.get("license").and_then(clean_simple_property) {
            metadata.license = Some(license);
        }
    }
    if metadata.repository.is_none() {
        if let Some(repository) = values
            .get("repository")
            .and_then(clean_simple_property)
        {
            metadata.repository = Some(repository);
        }
    }
}

fn clean_simple_property(raw: &String) -> Option<String> {
    let trimmed = raw.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    if trimmed.is_empty() || trimmed == "-" {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn query_properties_from_show(
    package: &str,
    properties: &[&str],
    remote: bool,
) -> Option<HashMap<String, String>> {
    if properties.is_empty() {
        return Some(HashMap::new());
    }

    let mut command = Command::new("xbps-query");
    if remote {
        command.arg("-R");
    } else {
        command.arg("-S");
    }
    command.arg("--show");
    command.arg(package);

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let property_set: HashSet<&str> = properties.iter().copied().collect();
    let mut result: HashMap<String, String> = HashMap::new();

    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for line in stdout.lines() {
        let trimmed_end = line.trim_end();
        if let Some((candidate, remainder)) = trimmed_end.split_once(':') {
            let key = candidate.trim();
            if property_set.contains(key) {
                if let Some(prev_key) = current_key.take() {
                    let normalized = normalize_property_text(&current_value);
                    result.entry(prev_key).or_insert(normalized);
                }
                current_key = Some(key.to_string());
                current_value = remainder.trim_start().to_string();
                continue;
            } else if current_key.is_some() {
                if let Some(prev_key) = current_key.take() {
                    let normalized = normalize_property_text(&current_value);
                    result.entry(prev_key).or_insert(normalized);
                }
                current_value.clear();
            }
        }

        if current_key.is_some() {
            let value = trimmed_end.trim();
            if value.is_empty() {
                continue;
            }
            if !current_value.is_empty() {
                current_value.push('\n');
            }
            current_value.push_str(value);
            continue;
        }
    }

    if let Some(prev_key) = current_key {
        let normalized = normalize_property_text(&current_value);
        result.entry(prev_key).or_insert(normalized);
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

fn normalize_property_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        String::new()
    } else if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn query_repo_package_info(name: &str) -> Result<PackageInfo, String> {
    let output = Command::new("xbps-query")
        .args(["-R", name])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut pkgver = String::new();
    let mut description = String::new();
    let mut pkgsize_bytes: Option<u64> = None;
    let mut download_literal: Option<String> = None;
    let mut changelog: Option<String> = None;
    let mut capture_changelog = false;

    for line in stdout.lines() {
        if let Some(value) = line.strip_prefix("pkgver:") {
            pkgver = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("short_desc:") {
            description = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("pkgsize:") {
            let trimmed = value.trim();
            if pkgsize_bytes.is_none() {
                pkgsize_bytes = parse_bytes_from_field(trimmed).or_else(|| parse_bytes(trimmed));
            }
            if download_literal.is_none() && !trimmed.is_empty() {
                download_literal = Some(trimmed.to_string());
            }
        } else if let Some(value) = line.strip_prefix("filename-size:") {
            let trimmed = value.trim();
            if pkgsize_bytes.is_none() {
                pkgsize_bytes = parse_bytes_from_field(trimmed).or_else(|| parse_bytes(trimmed));
            }
            if download_literal.is_none() && !trimmed.is_empty() {
                download_literal = Some(trimmed.to_string());
            }
        } else if let Some(value) = line.strip_prefix("changelog:") {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                capture_changelog = true;
            } else {
                changelog = Some(trimmed.to_string());
            }
        } else if capture_changelog {
            if line.starts_with(' ') || line.starts_with('\t') {
                let trimmed = line.trim();
                if !trimmed.is_empty() && changelog.is_none() {
                    changelog = Some(trimmed.to_string());
                }
            }
            capture_changelog = false;
        }
    }

    if description.is_empty() {
        description = "Update available".to_string();
    }

    let version = if !pkgver.is_empty() {
        let (_, ver) = split_package_identifier(&pkgver);
        ver
    } else {
        String::new()
    };

    let download_bytes = pkgsize_bytes;
    let download_size = download_bytes.map(format_size).or(download_literal);

    let name_owned = name.to_string();
    let version_lower = lowercase_cache(&version);
    let description_lower = lowercase_cache(&description);

    Ok(PackageInfo {
        name_lower: lowercase_cache(&name_owned),
        version_lower,
        description_lower,
        name: name_owned,
        version,
        description,
        installed: true,
        previous_version: None,
        download_size,
        changelog,
        download_bytes,
        repository: None,
        build_date: None,
        first_seen: None,
    })
}

pub(crate) fn query_installed_package_version(name: &str) -> Option<String> {
    let output = Command::new("xbps-query")
        .args(["-p", "pkgver", name])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let identifier = trimmed
            .strip_prefix("pkgver:")
            .map(|value| value.trim())
            .unwrap_or(trimmed);
        let (_name, version) = split_package_identifier(identifier);
        if !version.is_empty() {
            return Some(version);
        }
    }

    None
}

pub(crate) fn run_xbps_remove_orphans() -> Result<CommandResult, String> {
    run_privileged_command("xbps-remove", &["-O"])
}

pub(crate) fn run_xbps_pkgdb_check() -> Result<CommandResult, String> {
    run_privileged_command("xbps-pkgdb", &["-a"])
}

pub(crate) fn run_xbps_reconfigure_all() -> Result<CommandResult, String> {
    run_privileged_command("xbps-reconfigure", &["-a"])
}

pub(crate) fn run_xbps_alternatives_list() -> Result<CommandResult, String> {
    let output = Command::new("xbps-alternatives")
        .arg("-l")
        .output()
        .map_err(|err| format!("Failed to launch xbps-alternatives: {}", err))?;

    Ok(CommandResult {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

pub(crate) fn run_xbps_check_updates() -> Result<Vec<PackageInfo>, String> {
    let output = Command::new("xbps-install")
        .args(["-Sun"])
        .env("NO_COLOR", "1")
        .env("XBPS_INSTALL_VERBOSE", "2")
        .output()
        .map_err(|err| format!("Failed to launch xbps-install: {}", err))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let cleaned = strip_ansi_codes(&stdout);
    Ok(parse_updates_output(&cleaned))
}

pub(crate) fn run_xbps_update_all() -> Result<CommandResult, String> {
    run_privileged_command("xbps-install", &["-y", "-Su"])
}

pub(crate) fn run_xbps_update_package(package: &str) -> Result<CommandResult, String> {
    run_privileged_command("xbps-install", &["-y", "-u", package])
}

pub(crate) fn run_xbps_update_packages(packages: &[String]) -> Result<CommandResult, String> {
    if packages.is_empty() {
        return Ok(CommandResult {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    let mut args = vec!["-y", "-u"];
    let package_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
    args.extend(package_refs);
    run_privileged_command("xbps-install", &args)
}

fn parse_updates_output(text: &str) -> Vec<PackageInfo> {
    let mut updates = Vec::new();

    for raw_line in text.lines() {
        let mut line = raw_line.trim().trim_start_matches('\r');
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("xbps-install:") {
            line = rest.trim();
        }

        line = line
            .trim_start_matches(|c: char| c == '*' || c == '-' || c == '>')
            .trim();

        loop {
            if line.starts_with('[') {
                if let Some(pos) = line.find(']') {
                    line = line[pos + 1..].trim();
                    continue;
                }
            }
            break;
        }

        if line.is_empty() {
            continue;
        }

        if let Some(idx) = line.find("->") {
            let left = line[..idx].trim();
            let right = line[idx + 2..].trim();
            if left.is_empty() || right.is_empty() {
                continue;
            }

            let (name, prev_version) = split_package_identifier(left);
            if name.is_empty() {
                continue;
            }

            let new_version_token = right
                .split_whitespace()
                .find(|token| token.chars().any(|c| c.is_ascii_digit()))
                .unwrap_or("");
            let version = if new_version_token.contains('-') {
                let (_, ver) = split_package_identifier(new_version_token);
                ver
            } else {
                new_version_token.to_string()
            };

            add_update_entry(&mut updates, name, version, Some(prev_version));
            continue;
        }

        if line.contains("update available") {
            let (identifier, rest) = match line.split_once(" update available") {
                Some((id, remainder)) => (id.trim(), remainder.trim()),
                None => continue,
            };
            if identifier.is_empty() {
                continue;
            }

            let (name, version) = split_package_identifier(identifier);
            let previous_version = rest
                .split("(installed:")
                .nth(1)
                .and_then(|segment| segment.split(')').next())
                .map(|text| text.trim().to_string());

            add_update_entry(&mut updates, name, version, previous_version);
            continue;
        }

        if let Some(idx) = line.find(" update") {
            let left = line[..idx].trim();
            let right = line[idx + " update".len()..].trim();
            if left.is_empty() {
                continue;
            }

            let (name, prev_version) = split_package_identifier(left);
            if name.is_empty() {
                continue;
            }

            let new_version_token = right
                .split(|c| c == ')' || c == ' ' || c == ',' || c == ':')
                .find(|part| part.contains('-') || part.chars().any(|c| c.is_ascii_digit()))
                .unwrap_or("")
                .trim_start_matches('(')
                .trim();

            let version = if new_version_token.contains('-') {
                let (_, ver) = split_package_identifier(new_version_token);
                ver
            } else {
                new_version_token.to_string()
            };

            add_update_entry(&mut updates, name, version, Some(prev_version));
        }
    }

    updates.sort_by(|a, b| a.name.cmp(&b.name));
    updates
}

fn add_update_entry(
    updates: &mut Vec<PackageInfo>,
    name: String,
    version: String,
    previous_version: Option<String>,
) {
    if name.is_empty() {
        return;
    }

    let mut info = query_repo_package_info(&name).unwrap_or_else(|_| {
        let description = "Update available".to_string();
        PackageInfo {
            name_lower: lowercase_cache(&name),
            version_lower: lowercase_cache(&version),
            description_lower: lowercase_cache(&description),
            name: name.clone(),
            version: version.clone(),
            description,
            installed: true,
            previous_version: previous_version.clone(),
            download_size: None,
            changelog: None,
            download_bytes: None,
            repository: None,
            build_date: None,
            first_seen: None,
        }
    });

    if looks_like_version(&version) {
        info.set_version(version);
    }
    info.installed = true;
    if let Some(installed) = query_installed_package_version(&name) {
        info.previous_version = Some(installed);
    } else if let Some(prev) = previous_version {
        if !prev.is_empty() {
            info.previous_version = Some(prev);
        }
    }
    if info.download_size.is_none() {
        if let Some(bytes) = info.download_bytes {
            info.download_size = Some(format_size(bytes));
        }
    }

    updates.push(info);
}

fn looks_like_version(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let mut chars = text.chars();
    if let Some(first) = chars.next() {
        if first.is_ascii_digit() {
            return true;
        }
        if matches!(first, 'v' | 'r') {
            return chars.next().map(|c| c.is_ascii_digit()).unwrap_or(false);
        }
    }

    false
}

pub(crate) fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

pub(crate) fn format_download_size(bytes: u64) -> String {
    const KB: f64 = 1000.0;
    const MB: f64 = KB * 1000.0;
    const GB: f64 = MB * 1000.0;

    if bytes < MB as u64 {
        format!("{:.1} KB", bytes as f64 / KB)
    } else if bytes < GB as u64 {
        format!("{:.2} MB", bytes as f64 / MB)
    } else {
        format!("{:.2} GB", bytes as f64 / GB)
    }
}

pub(crate) fn summarize_output_line(text: &str) -> Option<String> {
    text.lines()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map(|line| truncate_for_summary(line, 96))
}

pub(crate) fn truncate_for_summary(text: &str, max_chars: usize) -> String {
    let mut result = String::new();
    let mut count = 0;
    let mut truncated = false;

    for ch in text.chars() {
        if count >= max_chars {
            truncated = true;
            break;
        }
        result.push(ch);
        count += 1;
    }

    if truncated {
        result.push_str("...");
    }

    result
}
