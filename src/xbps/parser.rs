use crate::types::{PackageInfo, lowercase_cache};

pub(crate) fn parse_bytes_from_field(text: &str) -> Option<u64> {
    let trimmed = text.trim().trim_end_matches(|c| c == ',' || c == '.');
    if trimmed.is_empty() {
        return None;
    }

    let cleaned = trimmed.replace(',', "");
    let mut parts = cleaned.split_whitespace();
    if let Some(first) = parts.next() {
        if let Ok(value) = first.parse::<u64>() {
            if let Some(unit) = parts.next() {
                return Some((value as f64 * unit_multiplier(unit)).round() as u64);
            }
            return Some(value);
        }
        if let Ok(value) = first.parse::<f64>() {
            if let Some(unit) = parts.next() {
                return Some((value * unit_multiplier(unit)).round() as u64);
            }
        }
    }

    let mut number = String::new();
    let mut unit = String::new();
    for ch in cleaned.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            number.push(ch);
        } else if !ch.is_whitespace() {
            unit.push(ch);
        }
    }

    if number.is_empty() {
        return None;
    }

    if unit.is_empty() {
        return number.parse::<u64>().ok();
    }

    let value = number.parse::<f64>().ok()?;
    Some((value * unit_multiplier(&unit)).round() as u64)
}

fn unit_multiplier(unit: &str) -> f64 {
    let cleaned = unit
        .trim()
        .trim_matches(|c: char| !c.is_ascii_alphabetic())
        .to_lowercase();
    match cleaned.as_str() {
        "b" | "byte" | "bytes" => 1.0,
        "k" | "kb" | "kib" | "ki" => 1024.0,
        "m" | "mb" | "mib" | "mi" => 1024.0 * 1024.0,
        "g" | "gb" | "gib" | "gi" => 1024.0 * 1024.0 * 1024.0,
        "t" | "tb" | "tib" | "ti" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        other if other.ends_with("ib") => match &other[..other.len() - 2] {
            "k" => 1024.0,
            "m" => 1024.0 * 1024.0,
            "g" => 1024.0 * 1024.0 * 1024.0,
            "t" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
            _ => 1.0,
        },
        _ => 1.0,
    }
}

pub(crate) fn parse_long_description(raw: &String) -> Option<String> {
    let mut lines = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        lines.push(trimmed.to_string());
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

pub(crate) fn parse_bytes(text: &str) -> Option<u64> {
    let cleaned = text
        .split_whitespace()
        .next()
        .unwrap_or(text)
        .trim()
        .trim_end_matches(|c: char| c == ',' || c == '.');
    cleaned.parse().ok()
}

pub(crate) fn parse_query_output(output: &str) -> Vec<PackageInfo> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            let mut tokens = trimmed.split_whitespace();
            let first = tokens.next()?;

            let (marker, identifier_token) = if first.starts_with('[') && first.ends_with(']') {
                (Some(first), tokens.next()?)
            } else {
                (None, first)
            };

            let mut installed = false;
            if let Some(marker) = marker {
                installed = marker.contains('x') || marker.contains('X');
            }

            let identifier = identifier_token.trim();
            let rest = tokens.collect::<Vec<_>>().join(" ");
            let (name, version) = split_package_identifier(identifier);

            let description = rest;
            Some(PackageInfo {
                name_lower: lowercase_cache(&name),
                version_lower: lowercase_cache(&version),
                description_lower: lowercase_cache(&description),
                name,
                version,
                description,
                installed,
                previous_version: None,
                download_size: None,
                changelog: None,
                download_bytes: None,
                repository: None,
                build_date: None,
                first_seen: None,
            })
        })
        .collect()
}

pub(crate) fn parse_installed_output(output: &str) -> Vec<PackageInfo> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }

            let mut split = trimmed.split_whitespace();
            let _status = split.next()?;
            let identifier = split.next()?;
            let (name, version) = split_package_identifier(identifier);

            let description_index = trimmed.find(identifier).map(|idx| idx + identifier.len());
            let description = description_index
                .and_then(|pos| trimmed.get(pos..))
                .map(|rest| rest.trim().to_string())
                .unwrap_or_default();

            Some(PackageInfo {
                name_lower: lowercase_cache(&name),
                version_lower: lowercase_cache(&version),
                description_lower: lowercase_cache(&description),
                name,
                version,
                description,
                installed: true,
                previous_version: None,
                download_size: None,
                changelog: None,
                download_bytes: None,
                repository: None,
                build_date: None,
                first_seen: None,
            })
        })
        .collect()
}

pub(crate) fn split_package_identifier(identifier: &str) -> (String, String) {
    if let Some(pos) = identifier.rfind('-') {
        let (name, version_part) = identifier.split_at(pos);
        (
            name.to_string(),
            version_part.trim_start_matches('-').to_string(),
        )
    } else {
        (identifier.to_string(), String::new())
    }
}

pub(crate) fn strip_ansi_codes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            continue;
        }
        if ch == '\u{1b}' {
            if matches!(chars.peek(), Some('[')) {
                chars.next();
                while let Some(next) = chars.next() {
                    if (next >= 'a' && next <= 'z') || (next >= 'A' && next <= 'Z') {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}
