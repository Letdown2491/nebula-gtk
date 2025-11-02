use std::collections::HashMap;
use std::process::Command;

use chrono::{DateTime, FixedOffset, LocalResult, NaiveDateTime, TimeZone, Utc};

use crate::xbps::split_package_identifier;

#[derive(Clone, Debug)]
pub struct RemotePackageMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repository: Option<String>,
    pub build_date: Option<DateTime<Utc>>,
}

pub(crate) fn fetch_remote_spotlight_metadata() -> Result<Vec<RemotePackageMetadata>, String> {
    let listings = Command::new("xbps-query")
        .args(["-R", "--regex", "-s", "."])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !listings.status.success() {
        let stderr = String::from_utf8_lossy(&listings.stderr);
        return Err(stderr.trim().to_string());
    }

    let build_dates = Command::new("xbps-query")
        .args(["-R", "--regex", "-s", ".", "-p", "build-date"])
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !build_dates.status.success() {
        let stderr = String::from_utf8_lossy(&build_dates.stderr);
        return Err(stderr.trim().to_string());
    }

    let mut records: HashMap<String, RemotePackageMetadata> = HashMap::new();

    for line in String::from_utf8_lossy(&listings.stdout).lines() {
        if let Some((name, version, description)) = parse_search_listing_line(line) {
            let entry = records
                .entry(name.clone())
                .or_insert_with(|| RemotePackageMetadata {
                    name: name.clone(),
                    version: version.clone(),
                    description: description.clone(),
                    repository: None,
                    build_date: None,
                });

            entry.version = version;
            if entry.description.is_empty() {
                entry.description = description;
            }
        }
    }

    for line in String::from_utf8_lossy(&build_dates.stdout).lines() {
        if let Some((name, version, build_date, repository)) = parse_build_date_listing_line(line) {
            let entry = records
                .entry(name.clone())
                .or_insert_with(|| RemotePackageMetadata {
                    name: name.clone(),
                    version: version.clone(),
                    description: String::new(),
                    repository: repository.clone(),
                    build_date: None,
                });

            entry.version = version;
            if entry.repository.is_none() {
                entry.repository = repository;
            }
            if build_date.is_some() {
                entry.build_date = build_date;
            }
        }
    }

    Ok(records.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_remote_spotlight_metadata_returns_packages() {
        let packages = fetch_remote_spotlight_metadata().expect("fetch spotlight metadata");
        assert!(
            !packages.is_empty(),
            "expected spotlight metadata to include packages"
        );
    }
}

fn parse_search_listing_line(line: &str) -> Option<(String, String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || !trimmed.starts_with('[') {
        return None;
    }

    let payload = trimmed.get(3..)?.trim_start();
    let mut split_index = None;
    for (idx, ch) in payload.char_indices() {
        if ch.is_whitespace() {
            split_index = Some(idx);
            break;
        }
    }

    let idx = split_index?;
    let identifier = payload[..idx].trim();
    if identifier.is_empty() {
        return None;
    }
    let description = payload[idx..].trim().to_string();
    let (name, version) = split_package_identifier(identifier);

    Some((name, version, description))
}

fn parse_build_date_listing_line(
    line: &str,
) -> Option<(String, String, Option<DateTime<Utc>>, Option<String>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (identifier, rest) = trimmed.split_once(':')?;
    let identifier = identifier.trim();
    if identifier.is_empty() {
        return None;
    }

    let mut remainder = rest.trim();
    let mut repository = None;
    if let Some(open_paren) = remainder.rfind('(') {
        if remainder.ends_with(')') && open_paren < remainder.len() {
            let repo_candidate = &remainder[open_paren + 1..remainder.len() - 1].trim();
            if !repo_candidate.is_empty() {
                repository = Some(repo_candidate.to_string());
            }
            remainder = remainder[..open_paren].trim_end();
        }
    }

    let build_date = parse_build_date_field(remainder);
    let (name, version) = split_package_identifier(identifier);
    Some((name, version, build_date, repository))
}

fn parse_build_date_field(value: &str) -> Option<DateTime<Utc>> {
    let trimmed = value.trim().trim_matches(|c| c == '"' || c == '\'');
    if trimmed.is_empty() {
        return None;
    }

    if let Some((date_part, tz_name)) = trimmed.rsplit_once(' ') {
        if tz_name.chars().all(|c| c.is_ascii_alphabetic()) {
            if let Some(offset) = timezone_offset_from_abbreviation(tz_name) {
                if let Some(result) = parse_with_fixed_offset(date_part.trim(), offset) {
                    return Some(result);
                }
            }
        }
    }

    let mut iso_candidate = trimmed.replace(" UTC", "Z");
    if !iso_candidate.contains('T') {
        iso_candidate = iso_candidate.replace(' ', "T");
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(&iso_candidate) {
        return Some(parsed.with_timezone(&Utc));
    }

    if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(Utc.from_utc_datetime(&parsed));
    }

    if let Ok(parsed) = NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M") {
        return Some(Utc.from_utc_datetime(&parsed));
    }

    None
}

fn parse_with_fixed_offset(date_part: &str, offset: FixedOffset) -> Option<DateTime<Utc>> {
    const FORMATS: [&str; 2] = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M"];

    for format in FORMATS {
        if let Ok(naive) = NaiveDateTime::parse_from_str(date_part, format) {
            match offset.from_local_datetime(&naive) {
                LocalResult::Single(dt) => return Some(dt.with_timezone(&Utc)),
                LocalResult::Ambiguous(first, second) => {
                    return Some(first.max(second).with_timezone(&Utc));
                }
                LocalResult::None => continue,
            }
        }
    }

    None
}

fn timezone_offset_from_abbreviation(name: &str) -> Option<FixedOffset> {
    match name {
        "UTC" | "GMT" => FixedOffset::east_opt(0),
        "CET" => FixedOffset::east_opt(3600),
        "CEST" => FixedOffset::east_opt(7200),
        "EET" => FixedOffset::east_opt(7200),
        "EEST" => FixedOffset::east_opt(10800),
        "PST" => FixedOffset::west_opt(8 * 3600),
        "PDT" => FixedOffset::west_opt(7 * 3600),
        "MST" => FixedOffset::west_opt(7 * 3600),
        "MDT" => FixedOffset::west_opt(6 * 3600),
        "CST" => FixedOffset::west_opt(6 * 3600),
        "CDT" => FixedOffset::west_opt(5 * 3600),
        "EST" => FixedOffset::west_opt(5 * 3600),
        "EDT" => FixedOffset::west_opt(4 * 3600),
        "BST" => FixedOffset::east_opt(3600),
        "IST" => FixedOffset::east_opt(19800),
        "JST" => FixedOffset::east_opt(9 * 3600),
        _ => None,
    }
}
