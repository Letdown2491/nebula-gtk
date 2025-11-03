use once_cell::sync::Lazy;
use std::process::Command;
use std::sync::RwLock;

use crate::xbps::run_privileged_command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MirrorTier {
    Tier1,
    Tor,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MirrorDefinition {
    pub(crate) id: &'static str,
    pub(crate) region: &'static str,
    pub(crate) base_url: &'static str,
    pub(crate) tier: MirrorTier,
}

const MIRRORS: &[MirrorDefinition] = &[
    MirrorDefinition {
        id: "repo-default",
        region: "Global",
        base_url: "https://repo-default.voidlinux.org",
        tier: MirrorTier::Tier1,
    },
    MirrorDefinition {
        id: "repo-fi",
        region: "Finland",
        base_url: "https://repo-fi.voidlinux.org",
        tier: MirrorTier::Tier1,
    },
    MirrorDefinition {
        id: "repo-de",
        region: "Germany",
        base_url: "https://repo-de.voidlinux.org",
        tier: MirrorTier::Tier1,
    },
    MirrorDefinition {
        id: "repo-fastly",
        region: "Global CDN",
        base_url: "https://repo-fastly.voidlinux.org",
        tier: MirrorTier::Tier1,
    },
    MirrorDefinition {
        id: "repo-us",
        region: "USA",
        base_url: "https://mirrors.servercentral.com/voidlinux",
        tier: MirrorTier::Tier1,
    },
    MirrorDefinition {
        id: "tor-se",
        region: "Sweden",
        base_url: "http://lysator7eknrfl47rlyxvgeamrv7ucefgrrlhk7rouv3sna25asetwid.onion/pub/voidlinux",
        tier: MirrorTier::Tor,
    },
    MirrorDefinition {
        id: "tor-dk",
        region: "Denmark",
        base_url: "http://dotsrccccbidkzg7oc7oj4ugxrlfbt64qebyunxbrgqhxiwj3nl6vcad.onion",
        tier: MirrorTier::Tor,
    },
];

const MAIN_SUFFIX: &str = "current";
const REPOSITORY_FILE: &str = "/etc/xbps.d/00-repository-main.conf";

static ACTIVE_REPOSITORIES: Lazy<RwLock<Vec<String>>> = Lazy::new(|| RwLock::new(Vec::new()));

pub(crate) fn tier1_mirrors() -> Vec<&'static MirrorDefinition> {
    MIRRORS
        .iter()
        .filter(|mirror| mirror.tier == MirrorTier::Tier1)
        .collect()
}

pub(crate) fn tor_mirrors() -> Vec<&'static MirrorDefinition> {
    MIRRORS
        .iter()
        .filter(|mirror| mirror.tier == MirrorTier::Tor)
        .collect()
}

pub(crate) fn default_mirror_id() -> &'static str {
    MIRRORS
        .iter()
        .find(|mirror| mirror.tier == MirrorTier::Tier1)
        .map(|mirror| mirror.id)
        .unwrap_or("repo-default")
}

pub(crate) fn find_mirror(id: &str) -> Option<&'static MirrorDefinition> {
    MIRRORS.iter().find(|mirror| mirror.id == id)
}

pub(crate) fn humanize_base_url(mirror: &MirrorDefinition) -> String {
    mirror
        .base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn repository_url(mirror: &MirrorDefinition, suffix: &str) -> String {
    let base = mirror.base_url.trim_end_matches('/');
    let suffix = suffix.trim_start_matches('/');
    format!("{}/{}", base, suffix)
}

pub(crate) fn configure_query_command(command: &mut Command) {
    let repos = active_repositories();
    if repos.is_empty() {
        return;
    }
    for repo in repos {
        command.arg("--repository");
        command.arg(repo);
    }
}

pub(crate) fn install_repository_args() -> Vec<String> {
    let repos = active_repositories();
    let mut args = Vec::with_capacity(repos.len() * 2);
    for repo in repos {
        args.push("-R".to_string());
        args.push(repo);
    }
    args
}

pub(crate) fn set_active_mirrors_by_ids(ids: &[String]) {
    let mut repos = Vec::new();
    for id in ids {
        if let Some(def) = find_mirror(id) {
            repos.push(repository_url(def, MAIN_SUFFIX));
        }
    }
    if let Ok(mut lock) = ACTIVE_REPOSITORIES.write() {
        *lock = repos;
    }
}

pub(crate) fn active_repositories() -> Vec<String> {
    match ACTIVE_REPOSITORIES.read() {
        Ok(repos) => repos.clone(),
        Err(_) => Vec::new(),
    }
}

pub(crate) fn detect_active_repositories() -> Result<Vec<String>, String> {
    let output = Command::new("xbps-query")
        .arg("-L")
        .output()
        .map_err(|err| format!("Failed to launch xbps-query: {}", err))?;

    if !output.status.success() {
        return Err(format!(
            "xbps-query -L failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut repositories = Vec::new();
    for line in stdout.lines() {
        if let Some(token) = line
            .split_whitespace()
            .find(|part| part.starts_with("http"))
        {
            repositories.push(token.trim_end_matches('/').to_string());
        }
    }

    Ok(repositories)
}

pub(crate) fn map_urls_to_ids(urls: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for definition in MIRRORS {
        let base = definition.base_url.trim_end_matches('/');
        if urls.iter().any(|url| {
            let normalized = url.trim_end_matches('/');
            normalized.starts_with(base)
        }) {
            result.push(definition.id.to_string());
        }
    }
    result
}

pub(crate) fn write_repository_config(ids: &[String]) -> Result<(), String> {
    let mirrors: Vec<&MirrorDefinition> = ids.iter().filter_map(|id| find_mirror(id)).collect();

    if mirrors.is_empty() {
        return Err("Select at least one mirror before saving.".to_string());
    }

    let content = mirrors
        .iter()
        .map(|mirror| format!("repository={}", repository_url(mirror, MAIN_SUFFIX)))
        .collect::<Vec<_>>()
        .join("\n");

    let script = format!("cat <<'EOF' > {REPOSITORY_FILE}\n{}\nEOF\n", content);
    let args: Vec<&str> = vec!["-c", script.as_str()];
    run_privileged_command("sh", &args)?;
    Ok(())
}
