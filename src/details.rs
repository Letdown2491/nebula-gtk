use crate::types::PackageInfo;

#[derive(Clone, Debug, Default)]
pub(crate) struct DiscoverDependency {
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DiscoverDetail {
    pub version: Option<String>,
    pub description: Option<String>,
    pub download: Option<String>,
    pub download_bytes: Option<u64>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub maintainer: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<DiscoverDependency>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct InstalledDetail {
    pub long_description: Option<String>,
    pub download_formatted: Option<String>,
    pub download_bytes: Option<u64>,
    pub download_error: Option<String>,
    pub homepage: Option<String>,
    pub maintainer: Option<String>,
    pub license: Option<String>,
    pub required_by: Vec<String>,
    pub required_by_error: Option<String>,
}

impl DiscoverDetail {
    pub(crate) fn with_dependencies(pkg: &PackageInfo, dependencies: Vec<String>) -> Self {
        Self {
            version: Some(pkg.version.clone()),
            description: Some(pkg.description.clone()),
            repository: pkg.repository.clone(),
            dependencies: dependencies
                .into_iter()
                .map(|name| DiscoverDependency { name })
                .collect(),
            ..Default::default()
        }
    }
}
