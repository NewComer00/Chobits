#![allow(dead_code)]
use directories::BaseDirs;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ╔═══════════════════════════════════════════════════════════════════════╗
// ║  CONFIG  — Single source of truth for every path and default.         ║
// ║  All crates depend on the chobits *library* target and import these.  ║
// ║                                                                       ║
// ║  Chobits is a portable, self-contained install:                       ║
// ║                                                                       ║
// ║    <chobits-root>/                                                    ║
// ║    ├── bin/         (chobits-start)                                   ║
// ║    ├── local/bin/   (chobits, chobits-bar, plugin .wasm, …)         ║
// ║    ├── config.toml  (user config)                                     ║
// ║    ├── .zellij/     (Zellij config/data; layout.kdl written here)    ║
// ║    ├── models/      (default location)                                ║
// ║    ├── logs/        (auto gen upon each run)                          ║
// ║                                                                       ║
// ╚═══════════════════════════════════════════════════════════════════════╝

/// Default Zellij layout.  Templates filled in at launch time:
///   `{chobits_bin}`          — absolute path to the `chobits` daemon binary
///   `{plugin_path}`          — absolute path to the `.wasm` file
///   `{interval_secs}`        — polling interval in seconds
///   `{max_bytes}`            — snapshot size cap passed to the plugin
///   `{live_ascii_args}`      — live-ascii CLI args built from `[live-ascii]`
///   `{live_ascii_bin}`       — absolute path to the bundled `live-ascii` binary
///   `{chobits_bar_bin}`      — absolute path to the bundled `chobits-bar` binary
///   `{snapshot_port}`        — HTTP port for plugin snapshot POSTs
pub const DEFAULT_LAYOUT_KDL: &str = r#"layout {
    pane size=1 borderless=true {
        plugin location="tab-bar"
    }
    pane split_direction="vertical" {
        pane size=1 borderless=true command="{chobits_bin}" {
            args "--quiet"
        }
        pane focus=true
        pane split_direction="horizontal" size="30%" {
            pane command="{live_ascii_bin}" name="LIVE-ASCII" {
                args {live_ascii_args}
            }
            pane command="{chobits_bar_bin}" size="30%" borderless=true
        }
        pane size=1 borderless=true {
            plugin location="file:{plugin_path}" {
                snapshot_port "{snapshot_port}"
                interval_secs "{interval_secs}"
                max_bytes "{max_bytes}"
            }
        }
    }
    pane size=1 borderless=true {
        plugin location="status-bar"
    }
}
"#;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub persona: PersonaConfig,
    #[serde(alias = "expressions")]
    pub idle: IdleConfig,
    pub snapshot: SnapshotConfig,
    #[allow(dead_code)]
    pub zellij: ZellijConfig,
    #[allow(dead_code)]
    #[serde(rename = "live-ascii")] // TOML [live-ascii] → Rust live_ascii
    pub live_ascii: LiveAsciiConfig,
    #[serde(default)]
    pub vts: VtsConfig,
    #[serde(default)]
    pub bar: BarConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BarConfig {
    /// TCP port chobits-bar listens on for text reactions.
    #[serde(default = "default_bar_port")]
    pub port: u16,
    /// Max number of text reactions kept in the chobits-bar scrollback.
    #[serde(default = "default_history_length")]
    pub history_length: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZellijConfig {
    #[serde(default = "default_zellij_config_dir")]
    #[allow(dead_code)]
    pub config_dir: PathBuf,
    #[serde(default = "default_zellij_data_dir")]
    #[allow(dead_code)]
    pub data_dir: PathBuf,
    #[serde(default = "default_layout")]
    #[allow(dead_code)]
    pub layout: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotConfig {
    /// Localhost HTTP port for `POST /snapshot` from the Zellij plugin.
    #[serde(default = "default_snapshot_port")]
    pub port: u16,
    /// Truncate incoming snapshot text to this many bytes (head + tail kept).
    #[serde(default = "default_max_snapshot_bytes")]
    pub max_bytes: usize,
    // Plugin pane polling interval (see `[snapshot] interval_secs`).
    #[serde(default = "default_interval_secs")]
    #[allow(dead_code)]
    pub interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_url")]
    pub url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PersonaConfig {
    /// The character's name (e.g. "Chi").  The system prompt begins with
    /// "You are {name}." — the user owns the name.
    #[serde(default = "default_persona_name")]
    pub name: String,
    /// A short description of the character's personality.  Format instructions
    /// and expression aliases come from `[vts.*_alias]` at startup — do not
    /// put JSON format details here.
    #[serde(default = "default_persona_description")]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct LiveAsciiConfig {
    #[serde(default = "default_live_ascii_model")]
    pub model_set: PathBuf,
    #[serde(default = "default_true")]
    pub enable_vts: bool,
    #[serde(default = "default_vts_port")]
    pub vts_port: u16,
    #[serde(default = "default_true")]
    pub enable_mouse: bool,
    #[serde(default = "default_true")]
    pub enable_physics: bool,
    #[serde(default = "default_image_protocol")]
    pub image_protocol: String,
    #[serde(default = "default_bg_color")]
    pub bg_color: String,
    #[serde(default = "default_scale")]
    pub scale: String,
    #[serde(default = "default_offset_x")]
    pub offset_x: String,
    #[serde(default = "default_offset_y")]
    pub offset_y: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct IdleConfig {
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct VtsConfig {
    #[serde(default = "default_vts_url")]
    pub url: String,
    #[serde(default = "default_vts_plugin_name")]
    pub plugin_name: String,
    #[serde(default = "default_vts_developer")]
    pub developer: String,
    #[serde(default = "default_vts_auth_token_path")]
    pub auth_token_path: PathBuf,
    #[serde(default = "default_vts_connect_timeout_secs")]
    pub connect_timeout_secs: u64,
    /// Friendly LLM labels mapped to one or more discovered expression hotkey keys.
    /// TOML: `happy = "exp_01"` or `happy = ["exp_01", "exp_02"]`.
    #[serde(default)]
    pub expression_alias: HashMap<String, AliasAllowList>,
    /// Friendly LLM labels mapped to one or more discovered motion hotkey keys.
    /// TOML values are VTS hotkey names (e.g. `"Idle #2"`) or slug keys (`idle_2`).
    #[serde(default)]
    pub motion_alias: HashMap<String, AliasAllowList>,
}

/// TOML alias target: a single key or an allow-list of keys.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum AliasAllowList {
    One(String),
    Many(Vec<String>),
}

impl AliasAllowList {
    pub fn keys(&self) -> Vec<String> {
        match self {
            AliasAllowList::One(key) => vec![key.clone()],
            AliasAllowList::Many(keys) => keys.clone(),
        }
    }
}

fn default_history_length() -> usize {
    50
}

fn default_snapshot_port() -> u16 {
    chobits_meta::DEFAULT_SNAPSHOT_PORT
}

fn default_bar_port() -> u16 {
    7879
}

fn default_vts_port() -> u16 {
    8001
}

fn default_vts_url() -> String {
    "ws://127.0.0.1:8001".into()
}

fn default_vts_plugin_name() -> String {
    "Chobits".into()
}

fn default_vts_developer() -> String {
    "Chobits".into()
}

fn default_vts_auth_token_path() -> PathBuf {
    chobits_dir().join(".chobits").join("vts_token.json")
}

fn default_vts_connect_timeout_secs() -> u64 {
    30
}

fn default_backend() -> String {
    "ollama".into()
}

fn default_url() -> String {
    "http://localhost:11434".into()
}

fn default_model() -> String {
    "qwen3:0.6b".into()
}

fn default_max_tokens() -> u32 {
    512
}

fn default_max_snapshot_bytes() -> usize {
    4096
}

fn default_persona_name() -> String {
    "Chi".into()
}

fn default_persona_description() -> String {
    "Curious and warm terminal companion. You speak in short, casual reactions —\
     one or two sentences max.  You genuinely care about what the user is working on."
        .into()
}

fn default_idle_timeout() -> u64 {
    30
}

fn default_interval_secs() -> u64 {
    10
}

fn default_true() -> bool {
    true
}

fn default_image_protocol() -> String {
    "halfblock".into()
}

fn default_live_ascii_model() -> PathBuf {
    PathBuf::new()
}

fn default_zellij_config_dir() -> PathBuf {
    chobits_dir().join(".zellij").join("config")
}

fn default_zellij_data_dir() -> PathBuf {
    chobits_dir().join(".zellij").join("data")
}

fn default_layout() -> String {
    // Template: {plugin_path} etc. are replaced at launch time.
    DEFAULT_LAYOUT_KDL.to_string()
}

fn default_bg_color() -> String {
    "rgba(0,0,0,0)".into()
}

fn default_scale() -> String {
    "100%".into()
}

fn default_offset_x() -> String {
    "0%".into()
}

fn default_offset_y() -> String {
    "0%".into()
}

/// Escape a path for embedding in a KDL double-quoted string.
fn escape_kdl_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "\\\\")
}

/// Runtime paths and snapshot settings substituted into a layout KDL template.
pub struct LayoutKdlParams<'a> {
    pub chobits_bin: &'a Path,
    pub plugin_wasm: &'a Path,
    pub interval_secs: u64,
    pub max_bytes: usize,
    pub snapshot_port: u16,
    pub live_ascii_args: &'a str,
    pub live_ascii_bin: &'a Path,
    pub chobits_bar_bin: &'a Path,
}

/// Build the final KDL layout, filling in runtime values:
/// - `{chobits_bin}`          — absolute path to the chobits daemon binary
/// - `{plugin_path}`          — absolute path to the WASM plugin
/// - `{interval_secs}`        — polling interval from config
/// - `{max_bytes}`            — snapshot truncation limit from config
/// - `{live_ascii_args}`      — args for the live-ascii command
/// - `{live_ascii_bin}`       — absolute path to the bundled live-ascii binary
/// - `{chobits_bar_bin}`      — absolute path to the bundled chobits-bar binary
/// - `{snapshot_port}`        — snapshot HTTP port from config
///
/// Binaries are referenced by absolute path (rather than a bare name looked
/// up on `$PATH`) so the generated layout works whether or not
/// `<chobits-root>/bin/` has been added to `PATH`.
pub fn build_layout_kdl_from(template: &str, params: &LayoutKdlParams<'_>) -> String {
    template
        .replace("{chobits_bin}", &escape_kdl_path(params.chobits_bin))
        .replace("{plugin_path}", &escape_kdl_path(params.plugin_wasm))
        .replace("{interval_secs}", &params.interval_secs.to_string())
        .replace("{max_bytes}", &params.max_bytes.to_string())
        .replace("{snapshot_port}", &params.snapshot_port.to_string())
        .replace("{live_ascii_args}", params.live_ascii_args)
        .replace("{live_ascii_bin}", &escape_kdl_path(params.live_ascii_bin))
        .replace(
            "{chobits_bar_bin}",
            &escape_kdl_path(params.chobits_bar_bin),
        )
}

pub fn build_layout_kdl(params: &LayoutKdlParams<'_>) -> String {
    build_layout_kdl_from(DEFAULT_LAYOUT_KDL, params)
}

/// Build the `args` line for live-ascii from config.
/// Generates individual tokens for the KDL `args` list:
///   `"<model>" "--vts" "--vts-port" "<port>" "--mouse" ...`
/// plus optional `--bg-color`, `--scale`, `--offsetx`, `--offsety`
#[allow(dead_code)]
pub fn live_ascii_args(cfg: &LiveAsciiConfig) -> String {
    let escaped_model = escape_kdl_path(&cfg.model_set);
    let mut parts = vec![format!("\"{}\"", escaped_model)];
    if cfg.enable_vts {
        parts.push("\"--vts\"".into());
        parts.push("\"--vts-port\"".into());
        parts.push(format!("\"{}\"", cfg.vts_port));
    }
    if cfg.enable_mouse {
        parts.push("\"--mouse\"".into());
    }
    if cfg.enable_physics {
        parts.push("\"--physics\"".into());
    }
    parts.push("\"--image-protocol\"".into());
    parts.push(format!("\"{}\"", cfg.image_protocol));
    if cfg.bg_color != default_bg_color() {
        parts.push("\"--bg-color\"".into());
        parts.push(format!("\"{}\"", cfg.bg_color));
    }
    if cfg.scale != default_scale() {
        parts.push("\"--scale\"".into());
        parts.push(format!("\"{}\"", cfg.scale));
    }
    if cfg.offset_x != default_offset_x() {
        parts.push("\"--offsetx\"".into());
        parts.push(format!("\"{}\"", cfg.offset_x));
    }
    if cfg.offset_y != default_offset_y() {
        parts.push("\"--offsety\"".into());
        parts.push(format!("\"{}\"", cfg.offset_y));
    }
    parts.join(" ")
}

/// Sentinel file placed at the root of a Chobits installation.
/// `chobits_dir()` walks up from the executable until it finds a directory
/// containing this file.
pub const ROOT_MARKER: &str = ".chobits-root";

/// `<chobits-root>/bin/` — directory holding this binary (`chobits-start`).
pub fn bin_dir() -> PathBuf {
    chobits_dir().join("bin")
}

/// `<chobits-root>/local/bin/` — directory holding bundled sibling executables
/// (`chobits`, `chobits-bar`, `live-ascii`, `zellij`, ...)
/// and the `chobits-zellij.wasm` plugin.
pub fn local_bin_dir() -> PathBuf {
    chobits_dir().join("local").join("bin")
}

/// Locate a bundled executable, searching in order:
/// 1. `<chobits-root>/local/bin/` — sibling executables and the wasm plugin
/// 2. `<chobits-root>/bin/`       — the launcher's own directory
/// 3. bare name, resolved via `$PATH` — fallback for `cargo run` during development
pub fn find_executable(name: &str) -> PathBuf {
    for dir in [local_bin_dir(), bin_dir()] {
        let with_ext = dir
            .join(name)
            .with_extension(std::env::consts::EXE_EXTENSION);
        if with_ext.exists() {
            return with_ext;
        }
        let no_ext = dir.join(name);
        if no_ext.exists() {
            return no_ext;
        }
    }
    PathBuf::from(name)
}

/// Locate `chobits-zellij.wasm`. Search order:
/// 1. `<chobits-root>/local/bin/` — packaged layout
/// 2. `<chobits-root>/bin/`       — fallback for unusual layouts
/// 3. Walk up from the exe to find a Cargo workspace's
///    `target/wasm32-wasip1/{debug,release}/` — useful for `cargo run`
pub fn find_wasm(name: &str) -> PathBuf {
    for dir in [local_bin_dir(), bin_dir()] {
        let candidate = dir.join(name);
        if candidate.exists() {
            return candidate;
        }
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let mut ancestor = dir;
            loop {
                for profile in &["debug", "release"] {
                    let candidate = ancestor
                        .join("target")
                        .join("wasm32-wasip1")
                        .join(profile)
                        .join(name);
                    if candidate.exists() {
                        return candidate;
                    }
                }
                match ancestor.parent() {
                    Some(p) => ancestor = p,
                    None => break,
                }
            }
        }
    }
    PathBuf::from(name)
}

/// Root directory of the portable Chobits installation.
///
/// Resolution order:
/// 1. `CHOBITS_ROOT` env var — overrides everything.
/// 2. Walk up from the executable, looking for a directory that contains a
///    `.chobits-root` marker file. This makes the install relocatable — the
///    whole `<chobits-root>/` tree can be moved or copied without breaking
///    paths, as long as the sentinel file stays at the root.
/// 3. Walk up from the executable looking for a `config.toml` or a directory
///    named `bin/` — covers old layouts and `cargo run` from the workspace root.
///    Falls back to `.` if nothing is found.
///
/// The sentinel is created by `build.sh` and serves as the definitive
/// anchor for all other paths (`config.toml`, `models/`,
/// `.zellij/`, `local/bin/`, `logs/`).
///
/// Result is cached in a `OnceLock` — the first call does the filesystem walk,
/// subsequent calls return the cached `PathBuf` instantly.
pub fn chobits_dir() -> PathBuf {
    static CACHE: OnceLock<PathBuf> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            if let Ok(root) = std::env::var("CHOBITS_ROOT") {
                return PathBuf::from(root);
            }
            if let Ok(exe) = std::env::current_exe() {
                // Phase 1 — look for the sentinel
                let mut dir = exe.parent().map(Path::to_path_buf);
                while let Some(d) = dir {
                    if d.join(ROOT_MARKER).exists() {
                        return d;
                    }
                    dir = d.parent().map(Path::to_path_buf);
                }

                // Phase 2 — fallback: any directory with config.toml or bin/
                let mut dir = exe.parent().map(Path::to_path_buf);
                while let Some(d) = dir {
                    if d.join("config.toml").exists() || d.join("bin").is_dir() {
                        return d;
                    }
                    dir = d.parent().map(Path::to_path_buf);
                }
            }
            // Absolute last resort
            PathBuf::from(".")
        })
        .clone()
}

/// `<chobits-root>/config.toml`
pub fn config_path() -> PathBuf {
    chobits_dir().join("config.toml")
}

/// `<chobits-root>/logs/`
pub fn log_dir() -> PathBuf {
    chobits_dir().join("logs")
}

/// `<chobits-root>/models/`
pub fn models_dir() -> PathBuf {
    chobits_dir().join("models")
}

/// Cross-platform home directory resolution. Only used to expand a leading
/// `~/` in user-supplied config paths (e.g. `model_set = "~/my-models/x"`) —
/// it no longer has anything to do with locating the Chobits install itself.
pub fn home_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|d| d.home_dir().to_path_buf())
}

/// Expand `~/` or `~` at the start of a path to the home directory.
pub fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

/// Resolve a path coming from `config.toml` (e.g. `[live-ascii] model_set`):
/// - `~` / `~/...`   → expanded against the home directory
/// - absolute path   → used as-is
/// - relative path   → resolved against `<chobits-root>/`, *not* the
///   process's current working directory, since launching the exe (e.g. by
///   double-click) can leave CWD pointed at `bin/` rather than the root.
pub fn warn_vts_port_mismatch(live_ascii: &LiveAsciiConfig, vts: &VtsConfig) {
    let Some(url_port) = vts_url_port(&vts.url) else {
        return;
    };
    if url_port != live_ascii.vts_port {
        eprintln!(
            "[config] [vts].url port ({url_port}) != [live-ascii].vts_port ({}) — keep them in sync",
            live_ascii.vts_port
        );
    }
}

fn vts_url_port(url: &str) -> Option<u16> {
    url.rsplit_once(':')?
        .1
        .trim_end_matches('/')
        .parse()
        .ok()
}

pub fn resolve_config_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s == "~" || s.starts_with("~/") {
        return expand_tilde(path);
    }
    if path.is_absolute() {
        return path.to_path_buf();
    }
    chobits_dir().join(path)
}

impl Default for VtsConfig {
    fn default() -> Self {
        Self {
            url: default_vts_url(),
            plugin_name: default_vts_plugin_name(),
            developer: default_vts_developer(),
            auth_token_path: default_vts_auth_token_path(),
            connect_timeout_secs: default_vts_connect_timeout_secs(),
            expression_alias: HashMap::new(),
            motion_alias: HashMap::new(),
        }
    }
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            port: default_bar_port(),
            history_length: default_history_length(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = config_path();
        let mut config: Config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            toml::from_str(&content)?
        } else {
            Config::default_config()
        };
        // Resolve paths relative to <chobits-root>, expanding `~` too.
        config.live_ascii.model_set = resolve_config_path(&config.live_ascii.model_set);
        config.vts.auth_token_path = resolve_config_path(&config.vts.auth_token_path);
        config.zellij.config_dir = resolve_config_path(&config.zellij.config_dir);
        config.zellij.data_dir = resolve_config_path(&config.zellij.data_dir);
        Ok(config)
    }

    pub fn default_config() -> Self {
        Config {
            llm: LlmConfig {
                backend: default_backend(),
                url: default_url(),
                model: default_model(),
                max_tokens: default_max_tokens(),
                api_key: String::new(),
            },
            snapshot: SnapshotConfig {
                port: default_snapshot_port(),
                max_bytes: default_max_snapshot_bytes(),
                interval_secs: default_interval_secs(),
            },
            persona: PersonaConfig {
                name: default_persona_name(),
                description: default_persona_description(),
            },
            idle: IdleConfig {
                idle_timeout_secs: default_idle_timeout(),
            },
            vts: VtsConfig {
                url: default_vts_url(),
                plugin_name: default_vts_plugin_name(),
                developer: default_vts_developer(),
                auth_token_path: default_vts_auth_token_path(),
                connect_timeout_secs: default_vts_connect_timeout_secs(),
                expression_alias: HashMap::new(),
                motion_alias: HashMap::new(),
            },
            zellij: ZellijConfig {
                config_dir: default_zellij_config_dir(),
                data_dir: default_zellij_data_dir(),
                layout: default_layout(),
            },
            live_ascii: LiveAsciiConfig {
                model_set: default_live_ascii_model(),
                enable_vts: default_true(),
                vts_port: default_vts_port(),
                enable_mouse: default_true(),
                enable_physics: default_true(),
                image_protocol: default_image_protocol(),
                bg_color: default_bg_color(),
                scale: default_scale(),
                offset_x: default_offset_x(),
                offset_y: default_offset_y(),
            },
            bar: BarConfig {
                port: default_bar_port(),
                history_length: default_history_length(),
            },
        }
    }
}

/// Read the `layout` field from a config.toml file, or return the embedded
/// default.  The layout lives under `[zellij]`:
///
/// ```toml
/// [zellij]
/// layout = """
/// layout { ... }
/// """
/// ```
/// Templates like `{plugin_path}` and `{interval_secs}` in user-provided
/// layouts are filled in later via `build_layout_kdl`.
#[allow(dead_code)]
pub fn load_layout_from_config(config_path: &PathBuf) -> String {
    match std::fs::read_to_string(config_path) {
        Ok(raw) => {
            // Find the [zellij] section, then look for `layout = """` within it
            if let Some(section_start) = raw.find("[zellij]") {
                let section_end = raw[section_start + 8..]
                    .find("\n[")
                    .map(|p| section_start + 8 + p)
                    .unwrap_or(raw.len());
                let section = &raw[section_start..section_end];
                for needle in &["layout = \"\"\"", "layout = '''"] {
                    let close = if needle.contains('"') {
                        "\"\"\""
                    } else {
                        "'''"
                    };
                    if let Some(start) = section.find(needle) {
                        let after = &section[start + needle.len()..];
                        if let Some(end) = after.find(close) {
                            return after[..end].trim().to_string();
                        }
                    }
                }
            }
            DEFAULT_LAYOUT_KDL.to_string()
        }
        Err(_) => DEFAULT_LAYOUT_KDL.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_config_file(contents: &str) -> PathBuf {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "chobits-config-test-{}-{}.toml",
            std::process::id(),
            id
        ));
        std::fs::write(&path, contents).expect("write temp config");
        path
    }

    #[test]
    fn build_layout_kdl_from_substitutes_and_escapes_paths() {
        let template =
            "cmd=\"{chobits_bin}\" wasm=\"{plugin_path}\" secs={interval_secs} args={live_ascii_args}";
        let params = LayoutKdlParams {
            chobits_bin: Path::new(r"C:\Chobits\bin\chobits.exe"),
            plugin_wasm: Path::new(r"C:\Chobits\local\bin\plugin.wasm"),
            interval_secs: 15,
            max_bytes: 4096,
            snapshot_port: chobits_meta::DEFAULT_SNAPSHOT_PORT,
            live_ascii_args: r#""model.json" "--vts" "--vts-port" "8001""#,
            live_ascii_bin: Path::new(r"C:\Chobits\local\bin\live-ascii.exe"),
            chobits_bar_bin: Path::new(r"C:\Chobits\local\bin\chobits-bar.exe"),
        };
        let out = build_layout_kdl_from(template, &params);
        assert!(out.contains(r"C:\\Chobits\\bin\\chobits.exe"));
        assert!(out.contains(r"C:\\Chobits\\local\\bin\\plugin.wasm"));
        assert!(out.contains("secs=15"));
        assert!(out.contains(r#""model.json" "--vts" "--vts-port" "8001""#));
    }

    #[test]
    fn live_ascii_args_includes_defaults() {
        let cfg = LiveAsciiConfig {
            model_set: PathBuf::from("models/hiyori.model3.json"),
            enable_vts: true,
            vts_port: 8001,
            enable_mouse: true,
            enable_physics: true,
            image_protocol: "halfblock".into(),
            bg_color: default_bg_color(),
            scale: default_scale(),
            offset_x: default_offset_x(),
            offset_y: default_offset_y(),
        };
        let args = live_ascii_args(&cfg);
        assert!(args.contains(r#""models/hiyori.model3.json""#));
        assert!(args.contains("\"--vts\""));
        assert!(args.contains("\"--vts-port\""));
        assert!(args.contains("\"8001\""));
        assert!(args.contains("\"--mouse\""));
        assert!(args.contains("\"--physics\""));
        assert!(args.contains("\"--image-protocol\""));
        assert!(args.contains("\"halfblock\""));
        assert!(!args.contains("--bg-color"));
    }

    #[test]
    fn live_ascii_args_includes_optional_overrides() {
        let cfg = LiveAsciiConfig {
            model_set: PathBuf::from("m.json"),
            enable_vts: false,
            vts_port: 8001,
            enable_mouse: false,
            enable_physics: false,
            image_protocol: "kitty".into(),
            bg_color: "rgba(1,2,3,4)".into(),
            scale: "200%".into(),
            offset_x: "10%".into(),
            offset_y: "20%".into(),
        };
        let args = live_ascii_args(&cfg);
        assert!(!args.contains("--vts"));
        assert!(!args.contains("--mouse"));
        assert!(!args.contains("--physics"));
        assert!(args.contains("\"kitty\""));
        assert!(args.contains("\"--bg-color\""));
        assert!(args.contains("\"rgba(1,2,3,4)\""));
        assert!(args.contains("\"--scale\""));
        assert!(args.contains("\"200%\""));
        assert!(args.contains("\"--offsetx\""));
        assert!(args.contains("\"--offsety\""));
    }

    #[test]
    fn expand_tilde_expands_home_prefix() {
        if let Some(home) = home_dir() {
            assert_eq!(expand_tilde(Path::new("~/models")), home.join("models"));
            assert_eq!(expand_tilde(Path::new("~")), home);
        }
    }

    #[test]
    fn load_layout_from_config_reads_zellij_section() {
        let path = temp_config_file(
            r#"
[zellij]
layout = """
layout {
    pane command="{chobits_bin}"
}
"""
"#,
        );
        let layout = load_layout_from_config(&path);
        assert!(layout.contains("pane command=\"{chobits_bin}\""));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_layout_from_config_falls_back_to_default() {
        let path = temp_config_file("[llm]\nbackend = \"ollama\"\n");
        let layout = load_layout_from_config(&path);
        assert!(layout.contains("plugin location=\"tab-bar\""));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn vts_alias_tables_parse_from_toml() {
        let cfg: VtsConfig = toml::from_str(
            r#"
url = "ws://127.0.0.1:8001"

[expression_alias]
happy = "exp_01"

[motion_alias]
idle = ["idle_0", "idle_1"]
wave = "tap_0"
"#,
        )
        .expect("parse config");
        assert_eq!(
            cfg.expression_alias.get("happy"),
            Some(&AliasAllowList::One("exp_01".into()))
        );
        assert_eq!(
            cfg.motion_alias.get("idle"),
            Some(&AliasAllowList::Many(vec!["idle_0".into(), "idle_1".into()]))
        );
        assert_eq!(
            cfg.motion_alias.get("wave"),
            Some(&AliasAllowList::One("tap_0".into()))
        );
    }
}
