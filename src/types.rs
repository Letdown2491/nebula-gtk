use chrono::{DateTime, Utc};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub installed: bool,
    pub pinned: bool,
    pub previous_version: Option<String>,
    pub download_size: Option<String>,
    pub changelog: Option<String>,
    pub download_bytes: Option<u64>,
    pub repository: Option<String>,
    pub build_date: Option<DateTime<Utc>>,
    pub first_seen: Option<DateTime<Utc>>,
    pub name_lower: Arc<str>,
    pub version_lower: Arc<str>,
    pub description_lower: Arc<str>,
}

pub(crate) fn lowercase_cache(value: &str) -> Arc<str> {
    if value.is_empty() {
        Arc::<str>::from("")
    } else {
        Arc::<str>::from(value.to_lowercase())
    }
}

impl PackageInfo {
    pub(crate) fn set_version(&mut self, version: String) {
        self.version = version;
        self.version_lower = lowercase_cache(&self.version);
    }

    pub(crate) fn set_description(&mut self, description: String) {
        self.description = description;
        self.description_lower = lowercase_cache(&self.description);
    }
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandResult {
    pub(crate) fn success(&self) -> bool {
        self.code.unwrap_or(-1) == 0
    }
}

#[derive(Clone, Debug)]
pub struct DependencyInfo {
    pub name: String,
}
