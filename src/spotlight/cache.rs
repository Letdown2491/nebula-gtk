use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{PackageInfo, lowercase_cache};

const SPOTLIGHT_CACHE_FILE: &str = "spotlight.json";
pub(crate) const SPOTLIGHT_CACHE_VERSION: u32 = 1;
pub(crate) const SPOTLIGHT_CACHE_MAX_ENTRIES: usize = 4096;

#[derive(Clone, Debug, Default)]
pub struct SpotlightCache {
    pub generated_at: Option<DateTime<Utc>>,
    pub packages: HashMap<String, PackageInfo>,
}

#[derive(Serialize, Deserialize)]
struct SpotlightCacheFile {
    version: u32,
    generated_at: Option<String>,
    packages: Vec<SpotlightCacheEntryData>,
}

#[derive(Serialize, Deserialize)]
struct SpotlightCacheEntryData {
    name: String,
    version: String,
    description: String,
    repository: Option<String>,
    build_date: Option<String>,
    first_seen: Option<String>,
}

pub(crate) fn load_spotlight_cache_from_disk() -> SpotlightCache {
    let Some(path) = spotlight_cache_path() else {
        return SpotlightCache::default();
    };

    let Ok(content) = fs::read_to_string(&path) else {
        return SpotlightCache::default();
    };

    let Ok(file) = serde_json::from_str::<SpotlightCacheFile>(&content) else {
        return SpotlightCache::default();
    };

    if file.version != SPOTLIGHT_CACHE_VERSION {
        return SpotlightCache::default();
    }

    let mut cache = SpotlightCache::default();
    cache.generated_at = file.generated_at.as_deref().and_then(parse_cached_datetime);

    for entry in file.packages {
        if entry.name.is_empty() {
            continue;
        }

        let build_date = entry.build_date.as_deref().and_then(parse_cached_datetime);
        let first_seen = entry.first_seen.as_deref().and_then(parse_cached_datetime);

        let name = entry.name;
        let version = entry.version;
        let description = entry.description;
        let repository = entry.repository;

        let info = PackageInfo {
            name_lower: lowercase_cache(&name),
            version_lower: lowercase_cache(&version),
            description_lower: lowercase_cache(&description),
            name,
            version,
            description,
            installed: false,
            previous_version: None,
            download_size: None,
            changelog: None,
            download_bytes: None,
            repository,
            build_date,
            first_seen,
        };

        cache.packages.insert(info.name.clone(), info);
    }

    prune_spotlight_cache(&mut cache);

    cache
}

pub(crate) fn save_spotlight_cache_to_disk(cache: &SpotlightCache) -> Result<(), String> {
    let Some(path) = spotlight_cache_path() else {
        return Err("Unable to determine spotlight cache directory".to_string());
    };

    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return Err(format!("Failed to create cache directory: {}", err));
        }
    }

    let packages: Vec<SpotlightCacheEntryData> = cache
        .packages
        .values()
        .map(|info| SpotlightCacheEntryData {
            name: info.name.clone(),
            version: info.version.clone(),
            description: info.description.clone(),
            repository: info.repository.clone(),
            build_date: info.build_date.as_ref().map(format_cached_datetime),
            first_seen: info.first_seen.as_ref().map(format_cached_datetime),
        })
        .collect();

    let file = SpotlightCacheFile {
        version: SPOTLIGHT_CACHE_VERSION,
        generated_at: cache.generated_at.as_ref().map(format_cached_datetime),
        packages,
    };

    let data = serde_json::to_string_pretty(&file)
        .map_err(|err| format!("Failed to serialize spotlight cache: {}", err))?;

    fs::write(&path, data).map_err(|err| format!("Failed to write spotlight cache: {}", err))
}

pub(crate) fn prune_spotlight_cache(cache: &mut SpotlightCache) {
    if cache.packages.len() <= SPOTLIGHT_CACHE_MAX_ENTRIES {
        return;
    }

    let mut entries: Vec<(String, Option<DateTime<Utc>>, Option<DateTime<Utc>>)> = cache
        .packages
        .iter()
        .map(|(name, info)| (name.clone(), info.build_date, info.first_seen))
        .collect();

    entries.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.0.cmp(&b.0))
    });

    for (name, _, _) in entries.into_iter().skip(SPOTLIGHT_CACHE_MAX_ENTRIES) {
        cache.packages.remove(&name);
    }
}

pub(crate) fn spotlight_cache_dir() -> Option<PathBuf> {
    if let Ok(custom) = env::var("NEBULA_STORE_CACHE_DIR") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }

    if let Ok(cache_home) = env::var("XDG_CACHE_HOME") {
        let trimmed = cache_home.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("nebula-gtk"));
        }
    }

    if let Ok(home) = env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join(".cache").join("nebula-gtk"));
        }
    }

    None
}

fn spotlight_cache_path() -> Option<PathBuf> {
    spotlight_cache_dir().map(|dir| dir.join(SPOTLIGHT_CACHE_FILE))
}

fn parse_cached_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn format_cached_datetime(value: &DateTime<Utc>) -> String {
    value.to_rfc3339()
}
