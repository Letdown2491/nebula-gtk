use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};

use crate::types::{PackageInfo, lowercase_cache};

use super::cache::{SpotlightCache, prune_spotlight_cache};
use super::categories::{SpotlightCategory, all_spotlight_categories, category_allowlist};
use super::metadata::{RemotePackageMetadata, fetch_remote_spotlight_metadata};

pub(crate) const SPOTLIGHT_WINDOW_DAYS: i64 = 7;
pub(crate) const SPOTLIGHT_RECENT_LIMIT: usize = 25;

#[derive(Clone, Debug)]
pub struct SpotlightRefreshOutcome {
    pub cache: SpotlightCache,
    pub recent: Vec<PackageInfo>,
    pub categories: HashMap<SpotlightCategory, Vec<PackageInfo>>,
    pub refreshed_at: DateTime<Utc>,
}

pub(crate) fn build_category_results(
    cache: &SpotlightCache,
) -> HashMap<SpotlightCategory, Vec<PackageInfo>> {
    let mut results = HashMap::new();

    for category in all_spotlight_categories() {
        let mut packages = Vec::new();
        for name in category_allowlist(*category) {
            if let Some(info) = cache.packages.get(*name) {
                packages.push(info.clone());
            }
        }
        results.insert(*category, packages);
    }

    results
}

pub(crate) fn compute_spotlight_sections(
    cache: &SpotlightCache,
    now: DateTime<Utc>,
) -> Vec<PackageInfo> {
    let window_start = now - Duration::days(SPOTLIGHT_WINDOW_DAYS);

    let mut recent: Vec<PackageInfo> = cache
        .packages
        .values()
        .filter(|pkg| pkg.build_date.map_or(false, |dt| dt >= window_start))
        .cloned()
        .collect();

    recent.sort_by(|a, b| {
        b.build_date
            .cmp(&a.build_date)
            .then_with(|| b.first_seen.cmp(&a.first_seen))
            .then_with(|| a.name.cmp(&b.name))
    });

    if recent.is_empty() {
        recent = cache.packages.values().cloned().collect();
        recent.sort_by(|a, b| {
            b.build_date
                .cmp(&a.build_date)
                .then_with(|| b.first_seen.cmp(&a.first_seen))
                .then_with(|| a.name.cmp(&b.name))
        });
    }
    recent.truncate(SPOTLIGHT_RECENT_LIMIT);

    recent
}

pub(crate) fn refresh_spotlight_cache(
    mut cache: SpotlightCache,
) -> Result<SpotlightRefreshOutcome, String> {
    let now = Utc::now();
    let remote_packages = fetch_remote_spotlight_metadata()?;

    for remote in remote_packages {
        if remote.name.is_empty() {
            continue;
        }

        let RemotePackageMetadata {
            name,
            version,
            description,
            repository,
            build_date,
        } = remote;

        let build_date_for_entry = build_date.clone();

        let entry = cache
            .packages
            .entry(name.clone())
            .or_insert_with(|| PackageInfo {
                name_lower: lowercase_cache(&name),
                version_lower: lowercase_cache(&version),
                description_lower: lowercase_cache(&description),
                name: name.clone(),
                version: version.clone(),
                description: description.clone(),
                installed: false,
                previous_version: None,
                download_size: None,
                changelog: None,
                download_bytes: None,
                repository: repository.clone(),
                build_date: build_date_for_entry.clone(),
                first_seen: Some(now),
            });

        let version_changed = entry.version != version;
        if version_changed {
            entry.previous_version = Some(entry.version.clone());
        }

        entry.set_version(version.clone());
        entry.set_description(description.clone());
        entry.repository = repository.clone();

        if let Some(date) = build_date_for_entry.clone() {
            entry.build_date = Some(date);
        }

        if entry.first_seen.is_none() {
            entry.first_seen = Some(now);
        }
    }

    prune_spotlight_cache(&mut cache);
    cache.generated_at = Some(now);

    let categories = build_category_results(&cache);

    let mut recent = compute_spotlight_sections(&cache, now);
    recent.truncate(SPOTLIGHT_RECENT_LIMIT);

    #[cfg(debug_assertions)]
    {
        eprintln!(
            "Spotlight refresh fetched {} packages; recent={}",
            cache.packages.len(),
            recent.len(),
        );
    }

    Ok(SpotlightRefreshOutcome {
        cache,
        recent,
        categories,
        refreshed_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_spotlight_cache_produces_spotlight_lists() {
        let cache = SpotlightCache::default();
        let outcome = refresh_spotlight_cache(cache).expect("refresh spotlight cache");
        assert!(
            !outcome.recent.is_empty(),
            "expected recent spotlight entries"
        );
        assert!(
            !outcome.categories.is_empty(),
            "expected spotlight categories"
        );
    }
}
