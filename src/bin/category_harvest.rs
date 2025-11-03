use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use walkdir::WalkDir;

const CATEGORY_NAMES: [&str; 19] = [
    "Books",
    "Browsers",
    "Chat",
    "Development",
    "Education",
    "E-mail",
    "Finance",
    "Gaming",
    "Graphics",
    "Kernels",
    "Music",
    "News",
    "Office",
    "Other",
    "Photos",
    "Productivity",
    "System",
    "Tools and Utilities",
    "Video",
];

#[allow(dead_code)]
#[derive(Clone, Copy)]
enum Field {
    Name,
    ShortDesc,
    Homepage,
    Maintainer,
    Depends,
    Path,
    TemplateCategory,
}

#[derive(Clone, Copy)]
struct Rule {
    field: Field,
    pattern: &'static str,
    weight: f32,
}

#[allow(dead_code)]
impl Rule {
    const fn name(pattern: &'static str, weight: f32) -> Self {
        Self {
            field: Field::Name,
            pattern,
            weight,
        }
    }

    const fn desc(pattern: &'static str, weight: f32) -> Self {
        Self {
            field: Field::ShortDesc,
            pattern,
            weight,
        }
    }

    const fn homepage(pattern: &'static str, weight: f32) -> Self {
        Self {
            field: Field::Homepage,
            pattern,
            weight,
        }
    }

    const fn maintainer(pattern: &'static str, weight: f32) -> Self {
        Self {
            field: Field::Maintainer,
            pattern,
            weight,
        }
    }

    const fn depends(pattern: &'static str, weight: f32) -> Self {
        Self {
            field: Field::Depends,
            pattern,
            weight,
        }
    }

    const fn path(pattern: &'static str, weight: f32) -> Self {
        Self {
            field: Field::Path,
            pattern,
            weight,
        }
    }

    const fn template_category(pattern: &'static str, weight: f32) -> Self {
        Self {
            field: Field::TemplateCategory,
            pattern,
            weight,
        }
    }
}

#[derive(Clone)]
struct CategorySpec {
    name: &'static str,
    rules: Vec<Rule>,
    floor: f32,
}

#[derive(Debug, Serialize, Clone)]
struct RankedCategory {
    category: String,
    score: f32,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PackageSuggestion {
    pkgname: String,
    category: String,
    score: f32,
    override_applied: bool,
    reasons: Vec<String>,
    alternatives: Vec<RankedCategory>,
    short_desc: Option<String>,
    homepage: Option<String>,
    template_path: String,
}

#[derive(Debug, Serialize)]
struct OutputSummaryEntry {
    category: String,
    packages: usize,
}

#[derive(Debug, Serialize)]
struct HarvestSummary {
    generated_at: String,
    total_packages: usize,
    overrides_applied: usize,
    summary: Vec<OutputSummaryEntry>,
}

#[derive(Debug, Serialize)]
struct HarvestOutput {
    metadata: HarvestSummary,
    packages: Vec<PackageSuggestion>,
}

#[derive(Debug, Default, Clone)]
struct PackageData {
    pkgname: String,
    short_desc: Option<String>,
    homepage: Option<String>,
    maintainer: Option<String>,
    template_path: String,
    template_categories: Vec<String>,
    depends: Vec<String>,
}

impl PackageData {
    fn lower_name(&self) -> String {
        self.pkgname.to_ascii_lowercase()
    }

    fn lower_short_desc(&self) -> Option<String> {
        self.short_desc.as_ref().map(|s| s.to_ascii_lowercase())
    }

    fn lower_homepage(&self) -> Option<String> {
        self.homepage.as_ref().map(|s| s.to_ascii_lowercase())
    }

    fn lower_maintainer(&self) -> Option<String> {
        self.maintainer.as_ref().map(|s| s.to_ascii_lowercase())
    }

    fn lower_path(&self) -> String {
        self.template_path.to_ascii_lowercase()
    }

    fn lower_template_categories(&self) -> Vec<String> {
        self.template_categories
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect()
    }

    fn lower_depends(&self) -> Vec<String> {
        self.depends
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect()
    }
}

static CATEGORY_SPECS: Lazy<Vec<CategorySpec>> = Lazy::new(|| {
    vec![
        CategorySpec {
            name: "Books",
            rules: vec![
                Rule::name("calibre", 7.0),
                Rule::name("foliate", 6.0),
                Rule::desc("ebook", 4.5),
                Rule::desc("epub", 4.0),
                Rule::desc("reader", 3.5),
                Rule::depends("calibre", 5.0),
                Rule::path("books", 3.0),
                Rule::path("ebook", 3.0),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Browsers",
            rules: vec![
                Rule::name("browser", 4.0),
                Rule::desc("web browser", 5.5),
                Rule::desc("browser", 4.5),
                Rule::name("firefox", 7.0),
                Rule::name("chrom", 6.5),
                Rule::name("webkit", 4.5),
                Rule::name("palemoon", 6.0),
                Rule::name("vivaldi", 6.0),
                Rule::name("falkon", 6.0),
                Rule::depends("webkit", 3.5),
                Rule::depends("firefox", 3.5),
                Rule::path("browser", 3.0),
            ],
            floor: 5.0,
        },
        CategorySpec {
            name: "Chat",
            rules: vec![
                Rule::name("chat", 5.5),
                Rule::desc("chat", 4.5),
                Rule::desc("messag", 4.0),
                Rule::name("matrix", 5.0),
                Rule::name("element", 5.5),
                Rule::name("discord", 6.0),
                Rule::name("slack", 5.0),
                Rule::name("telegram", 6.0),
                Rule::name("signal", 6.0),
                Rule::name("tox", 4.5),
                Rule::name("irc", 4.5),
                Rule::depends("libpurple", 4.0),
                Rule::depends("weechat", 4.0),
                Rule::path("chat", 3.5),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Development",
            rules: vec![
                Rule::desc("compiler", 4.5),
                Rule::desc("development", 4.0),
                Rule::desc("debugger", 4.0),
                Rule::desc("programming", 4.0),
                Rule::desc("sdk", 4.0),
                Rule::desc("toolchain", 4.0),
                Rule::name("gcc", 5.5),
                Rule::name("clang", 5.5),
                Rule::name("gdb", 5.0),
                Rule::name("lldb", 5.0),
                Rule::name("rust", 4.5),
                Rule::name("cargo", 4.5),
                Rule::name("cmake", 4.5),
                Rule::name("make", 4.0),
                Rule::name("meson", 4.5),
                Rule::name("ninja", 4.5),
                Rule::depends("gtk-doc", 3.5),
                Rule::depends("cmake", 3.5),
                Rule::path("/lang", 3.5),
                Rule::path("/devel", 3.5),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Education",
            rules: vec![
                Rule::desc("education", 5.0),
                Rule::desc("learn", 4.0),
                Rule::desc("math", 4.5),
                Rule::desc("science", 4.0),
                Rule::desc("chemistry", 4.5),
                Rule::desc("astronomy", 4.5),
                Rule::desc("geography", 4.5),
                Rule::name("khan", 5.0),
                Rule::name("anki", 6.0),
                Rule::name("stellarium", 6.0),
                Rule::depends("khan", 5.0),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "E-mail",
            rules: vec![
                Rule::name("mail", 5.0),
                Rule::name("email", 5.5),
                Rule::desc("email", 5.5),
                Rule::desc("mail", 5.0),
                Rule::name("imap", 4.5),
                Rule::name("smtp", 4.5),
                Rule::name("thunderbird", 6.5),
                Rule::name("geary", 6.5),
                Rule::name("mutt", 5.0),
                Rule::depends("notmuch", 4.5),
                Rule::depends("dovecot", 4.0),
                Rule::path("mail", 3.5),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Finance",
            rules: vec![
                Rule::desc("finance", 6.0),
                Rule::desc("bank", 5.0),
                Rule::desc("budget", 5.0),
                Rule::desc("account", 4.5),
                Rule::name("ledger", 5.0),
                Rule::name("gnucash", 6.5),
                Rule::name("kresus", 6.0),
                Rule::name("money", 4.5),
                Rule::depends("finance", 4.0),
                Rule::path("finance", 4.0),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Gaming",
            rules: vec![
                Rule::desc("game", 5.0),
                Rule::desc("gaming", 5.5),
                Rule::name("game", 5.0),
                Rule::name("doom", 4.5),
                Rule::name("quake", 4.5),
                Rule::name("steam", 6.5),
                Rule::name("lutris", 6.5),
                Rule::name("minetest", 6.0),
                Rule::name("supertux", 6.0),
                Rule::depends("sdl", 4.0),
                Rule::depends("openal", 3.5),
                Rule::depends("vulkan", 3.5),
                Rule::path("games", 4.5),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Graphics",
            rules: vec![
                Rule::desc("graphics", 5.5),
                Rule::desc("drawing", 5.0),
                Rule::desc("3d", 4.5),
                Rule::desc("render", 4.5),
                Rule::desc("cad", 4.5),
                Rule::name("inkscape", 6.5),
                Rule::name("blender", 6.5),
                Rule::name("krita", 6.5),
                Rule::name("gimp", 6.5),
                Rule::depends("opengl", 4.0),
                Rule::path("graphics", 4.0),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Kernels",
            rules: vec![
                Rule::name("linux", 7.0),
                Rule::name("kernel", 7.0),
                Rule::name("rt", 4.5),
                Rule::desc("kernel", 6.0),
                Rule::path("kernel", 5.5),
                Rule::path("linux", 5.5),
            ],
            floor: 5.5,
        },
        CategorySpec {
            name: "Music",
            rules: vec![
                Rule::desc("music", 5.5),
                Rule::desc("audio", 4.5),
                Rule::name("music", 5.0),
                Rule::name("player", 4.5),
                Rule::name("mix", 4.5),
                Rule::name("daw", 5.0),
                Rule::name("spotify", 6.5),
                Rule::name("clementine", 6.0),
                Rule::name("rhythmbox", 6.0),
                Rule::depends("alsa", 3.5),
                Rule::depends("pulseaudio", 3.5),
                Rule::path("audio", 4.0),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "News",
            rules: vec![
                Rule::desc("news", 6.0),
                Rule::desc("rss", 5.5),
                Rule::name("rss", 5.5),
                Rule::name("news", 6.0),
                Rule::name("feed", 4.5),
                Rule::depends("rss", 4.0),
                Rule::path("news", 4.0),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Office",
            rules: vec![
                Rule::desc("office", 5.5),
                Rule::desc("spreadsheet", 5.5),
                Rule::desc("word", 5.0),
                Rule::desc("presentation", 5.0),
                Rule::name("libreoffice", 7.0),
                Rule::name("onlyoffice", 6.5),
                Rule::name("calligra", 6.0),
                Rule::name("abiword", 6.0),
                Rule::name("gnumeric", 6.0),
                Rule::depends("libreoffice", 5.0),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Other",
            rules: vec![],
            floor: 0.0,
        },
        CategorySpec {
            name: "Photos",
            rules: vec![
                Rule::desc("photo", 6.0),
                Rule::desc("photograph", 5.5),
                Rule::name("photo", 6.0),
                Rule::name("darktable", 6.5),
                Rule::name("rawtherapee", 6.5),
                Rule::name("shotwell", 6.0),
                Rule::desc("camera", 5.0),
                Rule::path("photo", 4.0),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "Productivity",
            rules: vec![
                Rule::desc("productivity", 5.5),
                Rule::desc("task", 5.0),
                Rule::desc("todo", 5.0),
                Rule::desc("note", 4.5),
                Rule::desc("calendar", 4.5),
                Rule::name("planner", 5.0),
                Rule::name("organizer", 5.0),
                Rule::name("journal", 4.5),
                Rule::depends("todo", 4.0),
                Rule::path("productivity", 4.0),
            ],
            floor: 4.5,
        },
        CategorySpec {
            name: "System",
            rules: vec![
                Rule::desc("system", 4.0),
                Rule::desc("daemon", 4.0),
                Rule::desc("service", 4.0),
                Rule::desc("filesystem", 4.0),
                Rule::desc("kernel", 4.0),
                Rule::name("systemd", 5.0),
                Rule::name("elogind", 5.0),
                Rule::name("udev", 5.0),
                Rule::name("grub", 5.0),
                Rule::depends("systemd", 4.0),
                Rule::depends("elogind", 4.0),
                Rule::depends("udev", 4.0),
                Rule::path("system", 4.0),
            ],
            floor: 3.5,
        },
        CategorySpec {
            name: "Tools and Utilities",
            rules: vec![
                Rule::desc("utility", 4.5),
                Rule::desc("tool", 4.0),
                Rule::desc("command-line", 4.0),
                Rule::desc("cli", 4.0),
                Rule::name("util", 4.0),
                Rule::name("tool", 4.0),
                Rule::path("/utils", 3.5),
                Rule::path("/tools", 3.5),
            ],
            floor: 3.5,
        },
        CategorySpec {
            name: "Video",
            rules: vec![
                Rule::desc("video", 6.0),
                Rule::desc("media", 4.5),
                Rule::name("mpv", 6.5),
                Rule::name("vlc", 6.5),
                Rule::name("ffmpeg", 6.0),
                Rule::name("plex", 6.0),
                Rule::desc("stream", 5.0),
                Rule::depends("ffmpeg", 5.0),
                Rule::depends("gstreamer", 4.5),
                Rule::path("video", 4.0),
            ],
            floor: 4.5,
        },
    ]
});

#[derive(Debug, serde::Deserialize)]
struct OverrideCategory {
    packages: Vec<String>,
}

type OverrideTable = HashMap<String, OverrideCategory>;

fn main() -> Result<()> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let void_repo = repo_root.join("vendor/void-packages");
    if !void_repo.exists() {
        bail!(
            "void-packages repository is missing at {}",
            void_repo.display()
        );
    }

    let overrides = load_overrides(repo_root.join("data/category_overrides.toml"))?;
    let overrides_map = flatten_overrides(&overrides)?;

    let packages = harvest_packages(&void_repo.join("srcpkgs"))?;
    let (suggestions, overrides_used) = categorize_packages(&packages, &overrides_map);

    let output = build_output(suggestions, overrides_used);

    let output_path = repo_root
        .join("data")
        .join("generated")
        .join("category_suggestions.json");
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory {}", parent.display()))?;
    }

    let serialized = serde_json::to_string_pretty(&output)?;
    fs::write(&output_path, serialized)
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    println!(
        "Wrote {} package suggestions to {}",
        output.metadata.total_packages,
        output_path.display()
    );

    Ok(())
}

fn load_overrides(path: PathBuf) -> Result<Option<OverrideTable>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read override file {}", path.display()))?;
    let parsed: OverrideTable = toml::from_str(&raw)
        .with_context(|| format!("failed to parse overrides from {}", path.display()))?;
    Ok(Some(parsed))
}

fn flatten_overrides(overrides: &Option<OverrideTable>) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    if let Some(table) = overrides {
        for (category, entry) in table {
            ensure_valid_category(category)?;
            for pkg in &entry.packages {
                let pkg_lower = pkg.to_ascii_lowercase();
                map.insert(pkg_lower, category.clone());
            }
        }
    }
    Ok(map)
}

fn ensure_valid_category(category: &str) -> Result<()> {
    if CATEGORY_NAMES
        .iter()
        .any(|c| c.eq_ignore_ascii_case(category))
    {
        Ok(())
    } else {
        bail!("override references unknown category '{category}'");
    }
}

fn harvest_packages(srcpkgs: &Path) -> Result<Vec<PackageData>> {
    if !srcpkgs.exists() {
        bail!("srcpkgs directory missing at {}", srcpkgs.display());
    }

    let mut packages = Vec::new();
    for entry in WalkDir::new(srcpkgs)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.file_name() == "template")
    {
        let path = entry.path().to_path_buf();
        if let Some(pkg) = parse_template(&path, srcpkgs) {
            packages.push(pkg);
        }
    }

    Ok(packages)
}

fn parse_template(path: &Path, srcpkgs: &Path) -> Option<PackageData> {
    let raw = fs::read_to_string(path).ok()?;
    let pkgname = extract_assignment(&raw, "pkgname")?;
    let short_desc = extract_assignment(&raw, "short_desc");
    let homepage = extract_assignment(&raw, "homepage");
    let maintainer = extract_assignment(&raw, "maintainer");
    let template_categories =
        extract_assignment(&raw, "categories").map(|value| parse_list(&value));
    let depends = collect_dep_fields(&raw);

    let rel_path = path
        .strip_prefix(srcpkgs)
        .ok()
        .and_then(|rel| rel.parent().map(|p| p.display().to_string()))
        .unwrap_or_default();

    Some(PackageData {
        pkgname,
        short_desc,
        homepage,
        maintainer,
        template_path: rel_path,
        template_categories: template_categories.unwrap_or_default(),
        depends,
    })
}

fn collect_dep_fields(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for key in [
        "depends",
        "run_depends",
        "hostmakedepends",
        "makedepends",
        "checkdepends",
        "subpackages",
    ] {
        if let Some(value) = extract_assignment(raw, key) {
            out.extend(parse_list(&value));
        }
    }
    let mut unique = HashSet::new();
    out.retain(|value| unique.insert(value.clone()));
    out
}

fn parse_list(raw: &str) -> Vec<String> {
    raw.replace(['\n', '\r', '\t'], " ")
        .split_whitespace()
        .filter_map(sanitize_token)
        .collect()
}

fn sanitize_token(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    if token.starts_with('$') {
        return None;
    }
    let mut trimmed =
        token.trim_matches(|c: char| c == '"' || c == '\'' || c == '`' || c == ',' || c == ';');
    trimmed = trimmed.trim_start_matches("${").trim_end_matches('}');
    trimmed = trimmed.trim_matches(|c: char| c == '(' || c == ')');
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('\"') {
        trimmed = trimmed.trim_start_matches('\"');
    }
    let token = trimmed
        .split(|c| matches!(c, '<' | '>' | '=' | '(' | '[' | '{' | ')'))
        .next()
        .unwrap_or(trimmed)
        .trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '+' || c == '.'))
        .trim()
        .to_string();
    if token.is_empty() { None } else { Some(token) }
}

fn extract_assignment(raw: &str, key: &str) -> Option<String> {
    static KEY_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?m)^(?P<key>[A-Za-z0-9_]+)\s*=\s*(?P<value>.+)$").expect("valid regex")
    });

    let mut cursor = 0usize;
    while let Some(mat) = KEY_REGEX.find_at(raw, cursor) {
        let line = mat.as_str();
        if let Some((found_key, _)) = line.split_once('=') {
            if found_key.trim() == key {
                return Some(read_value(raw, mat.start() + found_key.len() + 1));
            }
        }
        cursor = mat.end();
    }
    None
}

fn read_value(raw: &str, mut offset: usize) -> String {
    let bytes = raw.as_bytes();
    while let Some(b) = bytes.get(offset) {
        if b.is_ascii_whitespace() && *b != b'\n' && *b != b'\r' {
            offset += 1;
        } else {
            break;
        }
    }

    if let Some(b'"') = raw.as_bytes().get(offset) {
        read_quoted_value(raw, offset + 1, b'"')
    } else if let Some(b'\'') = raw.as_bytes().get(offset) {
        read_quoted_value(raw, offset + 1, b'\'')
    } else {
        let rest = &raw[offset..];
        let end = rest
            .find('\n')
            .or_else(|| rest.find('\r'))
            .unwrap_or(rest.len());
        rest[..end].trim().to_string()
    }
}

fn read_quoted_value(raw: &str, mut offset: usize, quote: u8) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::new();
    while let Some(&b) = bytes.get(offset) {
        offset += 1;
        if b == quote {
            if let Some(prev) = bytes.get(offset - 2) {
                if *prev == b'\\' {
                    out.push(b as char);
                    continue;
                }
            }
            break;
        }
        out.push(b as char);
    }
    out
}

fn categorize_packages(
    packages: &[PackageData],
    overrides: &HashMap<String, String>,
) -> (Vec<PackageSuggestion>, usize) {
    let mut output = Vec::with_capacity(packages.len());
    let mut overrides_used = 0usize;
    for pkg in packages {
        let (suggestion, override_hit) = categorize_package(pkg, overrides);
        if override_hit {
            overrides_used += 1;
        }
        output.push(suggestion);
    }
    output.sort_by(|a, b| a.pkgname.cmp(&b.pkgname));
    (output, overrides_used)
}

fn categorize_package(
    pkg: &PackageData,
    overrides: &HashMap<String, String>,
) -> (PackageSuggestion, bool) {
    let pkg_lower = pkg.lower_name();
    if let Some(category) = overrides.get(&pkg_lower) {
        return (
            PackageSuggestion {
                pkgname: pkg.pkgname.clone(),
                category: canonical_category(category),
                score: f32::INFINITY,
                override_applied: true,
                reasons: vec![format!("override → {category}")],
                alternatives: Vec::new(),
                short_desc: pkg.short_desc.clone(),
                homepage: pkg.homepage.clone(),
                template_path: pkg.template_path.clone(),
            },
            true,
        );
    }

    let lower_short_desc = pkg.lower_short_desc();
    let lower_homepage = pkg.lower_homepage();
    let lower_maintainer = pkg.lower_maintainer();
    let lower_path = pkg.lower_path();
    let lower_depends = pkg.lower_depends();
    let lower_template_categories = pkg.lower_template_categories();

    let mut ranked = Vec::new();

    for spec in CATEGORY_SPECS.iter().filter(|spec| spec.name != "Other") {
        let mut score = 0.0;
        let mut reasons = Vec::new();
        for rule in &spec.rules {
            if rule_matches(
                rule,
                &pkg_lower,
                lower_short_desc.as_deref(),
                lower_homepage.as_deref(),
                lower_maintainer.as_deref(),
                &lower_depends,
                &lower_path,
                &lower_template_categories,
            ) {
                score += rule.weight;
                reasons.push(format!(
                    "{} contains '{}'",
                    field_name(rule.field),
                    rule.pattern
                ));
            }
        }

        if score >= spec.floor {
            ranked.push(RankedCategory {
                category: spec.name.to_string(),
                score,
                reasons,
            });
        }
    }

    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let (category, score, reasons, alternatives) = if let Some(best) = ranked.first() {
        let mut alternatives = Vec::new();
        for candidate in ranked.iter().skip(1).take(4) {
            alternatives.push(candidate.clone());
        }
        (
            best.category.clone(),
            best.score,
            best.reasons.clone(),
            alternatives,
        )
    } else {
        (
            "Other".to_string(),
            0.0,
            vec!["no heuristic match".to_string()],
            Vec::new(),
        )
    };

    (
        PackageSuggestion {
            pkgname: pkg.pkgname.clone(),
            category,
            score,
            override_applied: false,
            reasons,
            alternatives,
            short_desc: pkg.short_desc.clone(),
            homepage: pkg.homepage.clone(),
            template_path: pkg.template_path.clone(),
        },
        false,
    )
}

fn canonical_category(name: &str) -> String {
    CATEGORY_NAMES
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(name))
        .map(|s| s.to_string())
        .unwrap_or_else(|| name.to_string())
}

fn rule_matches(
    rule: &Rule,
    name: &str,
    short_desc: Option<&str>,
    homepage: Option<&str>,
    maintainer: Option<&str>,
    depends: &[String],
    path: &str,
    template_categories: &[String],
) -> bool {
    let pattern = rule.pattern.to_ascii_lowercase();
    match rule.field {
        Field::Name => name.contains(&pattern),
        Field::ShortDesc => short_desc.map(|s| s.contains(&pattern)).unwrap_or(false),
        Field::Homepage => homepage.map(|s| s.contains(&pattern)).unwrap_or(false),
        Field::Maintainer => maintainer.map(|s| s.contains(&pattern)).unwrap_or(false),
        Field::Depends => depends.iter().any(|dep| dep.contains(&pattern)),
        Field::Path => path.contains(&pattern),
        Field::TemplateCategory => template_categories.iter().any(|c| c.contains(&pattern)),
    }
}

fn field_name(field: Field) -> &'static str {
    match field {
        Field::Name => "pkgname",
        Field::ShortDesc => "short_desc",
        Field::Homepage => "homepage",
        Field::Maintainer => "maintainer",
        Field::Depends => "dependencies",
        Field::Path => "template path",
        Field::TemplateCategory => "template category",
    }
}

fn build_output(suggestions: Vec<PackageSuggestion>, overrides_count: usize) -> HarvestOutput {
    let mut summary = BTreeMap::<String, usize>::new();
    for suggestion in &suggestions {
        *summary.entry(suggestion.category.clone()).or_default() += 1;
    }
    let summary_vec = summary
        .into_iter()
        .map(|(category, packages)| OutputSummaryEntry { category, packages })
        .collect();

    let generated_at = SystemTime::now();
    let generated_at = chrono::DateTime::<chrono::Utc>::from(generated_at)
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    HarvestOutput {
        metadata: HarvestSummary {
            generated_at,
            total_packages: suggestions.len(),
            overrides_applied: overrides_count,
            summary: summary_vec,
        },
        packages: suggestions,
    }
}
