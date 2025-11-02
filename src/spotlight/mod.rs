mod cache;
mod categories;
mod metadata;
mod refresh;

pub(crate) use cache::{
    SpotlightCache, load_spotlight_cache_from_disk, save_spotlight_cache_to_disk,
};
pub(crate) use categories::{SpotlightCategory, category_display_name};
pub(crate) use refresh::{
    build_category_results, compute_spotlight_sections, refresh_spotlight_cache,
};

pub(crate) const SPOTLIGHT_REFRESH_INTERVAL_HOURS: i64 = 24;
