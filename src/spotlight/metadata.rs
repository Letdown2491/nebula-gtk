use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::process::Command;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, LocalResult, NaiveDateTime, TimeZone, Utc};
use feed_rs::parser;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};

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
    match fetch_remote_spotlight_metadata_from_feed() {
        Ok(entries) if !entries.is_empty() => Ok(entries),
        Ok(_) => fetch_remote_spotlight_metadata_with_xbps(),
        Err(feed_err) => {
            eprintln!("Failed to refresh spotlight feed: {}", feed_err);
            fetch_remote_spotlight_metadata_with_xbps()
                .map_err(|xbps_err| format!("{feed_err}; fallback failed: {xbps_err}"))
        }
    }
}

const VOID_PACKAGES_ATOM_URL: &str =
    "https://github.com/void-linux/void-packages/commits/master.atom";
const HTTP_TIMEOUT_SECS: u64 = 10;

fn fetch_remote_spotlight_metadata_from_feed() -> Result<Vec<RemotePackageMetadata>, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|err| format!("Failed to build HTTP client: {}", err))?;

    let response = client
        .get(VOID_PACKAGES_ATOM_URL)
        .header(ACCEPT, "application/atom+xml")
        .header(
            USER_AGENT,
            "Nebula/0.8.7 (https://github.com/void-linux/void-packages)",
        )
        .send()
        .map_err(|err| format!("Failed to request Atom feed: {}", err))?;

    if !response.status().is_success() {
        return Err(format!(
            "Atom feed returned HTTP {}",
            response.status().as_u16()
        ));
    }

    let bytes = response
        .bytes()
        .map_err(|err| format!("Failed to read Atom feed: {}", err))?;
    let mut cursor = Cursor::new(bytes);
    let feed =
        parser::parse(&mut cursor).map_err(|err| format!("Failed to parse Atom feed: {}", err))?;

    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for entry in feed.entries {
        let Some(title) = entry.title.as_ref().map(|t| t.content.trim().to_string()) else {
            continue;
        };

        let Some(package_name) = extract_package_name(&title) else {
            continue;
        };

        if !seen.insert(package_name.clone()) {
            continue;
        }

        let version = extract_version_hint(&title).unwrap_or_default();
        let description = entry
            .summary
            .as_ref()
            .map(|s| s.content.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| title.clone());
        let timestamp = entry
            .updated
            .or(entry.published)
            .map(|dt| dt.with_timezone(&Utc));

        results.push(RemotePackageMetadata {
            name: package_name,
            version,
            description,
            repository: Some("void-packages".to_string()),
            build_date: timestamp,
        });
    }

    if results.is_empty() {
        Err("Atom feed did not contain recognizable package updates".to_string())
    } else {
        Ok(results)
    }
}

fn fetch_remote_spotlight_metadata_with_xbps() -> Result<Vec<RemotePackageMetadata>, String> {
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

fn extract_package_name(title: &str) -> Option<String> {
    if let Some(idx) = title.find("srcpkgs/") {
        let mut name = String::new();
        for ch in title[idx + "srcpkgs/".len()..].chars() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '+' | '.') {
                name.push(ch);
            } else {
                break;
            }
        }
        if !name.is_empty() {
            return Some(name);
        }
    }

    if let Some(idx) = title.find(':') {
        let prefix = title[..idx].trim();
        for part in prefix.split(|c: char| c == ',' || c.is_whitespace()) {
            if part.is_empty() {
                continue;
            }
            let candidate = part
                .split('/')
                .last()
                .unwrap_or(part)
                .trim_matches(|c: char| {
                    !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.'
                });
            if !candidate.is_empty() {
                return Some(candidate.to_string());
            }
        }
    }

    title
        .split_whitespace()
        .next()
        .map(|word| {
            word.trim_matches(|c: char| {
                !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '.'
            })
            .to_string()
        })
        .filter(|s| !s.is_empty())
}

fn extract_version_hint(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    for marker in ["update to ", "bump to ", "to version "] {
        if let Some(idx) = lower.find(marker) {
            let start = idx + marker.len();
            let slice = &title[start..];
            let version: String = slice
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | '+'))
                .collect();
            if !version.is_empty() {
                return Some(version);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
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
