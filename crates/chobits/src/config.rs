#![allow(dead_code)]
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use directories::BaseDirs;

// ╔═══════════════════════════════════════════════════════════════════════╗
// ║  CONFIG  — Single source of truth for every path and default.         ║
// ║  All crates depend on the chobits *library* target and import these.  ║
// ║                                                                       ║
// ║  Chobits is a portable, self-contained install:                       ║
// ║                                                                       ║
// ║    <chobits-root>/                                                    ║
// ║    ├── bin/         (this binary + sibling executables + .wasm)       ║
// ║    ├── config.toml  (user config)                                     ║
// ║    ├── expressions/ (default location)                                ║
// ║    ├── layout.kdl   (auto gen upon each run)                          ║
// ║    ├── logs/        (auto gen upon each run)                          ║
// ║    └── models/      (default location)                                ║
// ║                                                                       ║
// ╚═══════════════════════════════════════════════════════════════════════╝

/// Default Zellij layout.  Templates filled in at launch time:
///   `{chobits_bin}`          — absolute path to the `chobits` daemon binary
///   `{plugin_path}`          — absolute path to the `.wasm` file
///   `{interval_secs}`        — polling interval in seconds
///   `{live_ascii_args}`      — live-ascii CLI args built from `[live-ascii]`
///   `{live_ascii_bin}`       — absolute path to the bundled `live-ascii` binary
///   `{chobits_bar_bin}`      — absolute path to the bundled `chobits-bar` binary
///   `{chobits_send_bin}`     — absolute path to the bundled `chobits-send` binary
///   `{zellij_bin}`           — absolute path to the bundled `zellij` binary
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
                zellij_bin "{zellij_bin}"
                chobits_send_bin "{chobits_send_bin}"
                interval_secs "{interval_secs}"
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
    #[allow(dead_code)]
    pub expressions: ExpressionsConfig,
    pub snapshot: SnapshotConfig,
    #[allow(dead_code)]
    pub zellij: ZellijConfig,
    #[allow(dead_code)]
    #[serde(rename = "live-ascii")]  // TOML [live-ascii] → Rust live_ascii
    pub live_ascii: LiveAsciiConfig,
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
    /// TCP port the daemon listens on for terminal snapshots.
    #[serde(default = "default_snapshot_port")]
    pub port: u16,
    /// Truncate incoming snapshot text to this many bytes (head + tail kept).
    #[serde(default = "default_max_snapshot_bytes")]
    pub max_bytes: usize,
    // Dump-screen polling interval
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
    /// A short description of the character's personality.  Format instructions,
    /// the expression list, and the JSON template are all generated automatically
    /// from the filesystem — the user should never put those here.
    #[serde(default = "default_persona_description")]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct LiveAsciiConfig {
    #[serde(default = "default_live_ascii_model")]
    pub model_set: PathBuf,
    #[serde(default = "default_true")]
    pub enable_osf: bool,
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
pub struct ExpressionsConfig {
    #[serde(default = "default_expressions_dir")]
    pub dir: PathBuf,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    /// UDP port live-ascii listens on for OSF expression frames.
    #[serde(default = "default_osf_port")]
    pub osf_port: u16,
}

fn default_history_length() -> usize {
    50
}

fn default_snapshot_port() -> u16 {
    7878
}

fn default_bar_port() -> u16 {
    7879
}

fn default_osf_port() -> u16 {
    11573
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

fn default_expressions_dir() -> PathBuf {
    expressions_dir()
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

/// Build the final KDL layout, filling in runtime values:
/// - `{chobits_bin}`          — absolute path to the chobits daemon binary
/// - `{plugin_path}`          — absolute path to the WASM plugin
/// - `{interval_secs}`        — polling interval from config
/// - `{live_ascii_args}`      — args for the live-ascii command
/// - `{live_ascii_bin}`       — absolute path to the bundled live-ascii binary
/// - `{chobits_bar_bin}`      — absolute path to the bundled chobits-bar binary
/// - `{chobits_send_bin}`     — absolute path to the bundled chobits-send binary
/// - `{zellij_bin}`           — absolute path to the bundled zellij binary
///
/// Binaries are referenced by absolute path (rather than a bare name looked
/// up on `$PATH`) so the generated layout works whether or not
/// `<chobits-root>/bin/` has been added to `PATH`.
pub fn build_layout_kdl_from(
    template: &str,
    chobits_bin: &Path,
    plugin_wasm: &Path,
    interval_secs: u64,
    live_ascii_args: &str,
    live_ascii_bin: &Path,
    chobits_bar_bin: &Path,
    chobits_send_bin: &Path,
    zellij_bin: &Path,
) -> String {
    template
        .replace("{chobits_bin}", &escape_kdl_path(chobits_bin))
        .replace("{plugin_path}", &escape_kdl_path(plugin_wasm))
        .replace("{interval_secs}", &interval_secs.to_string())
        .replace("{live_ascii_args}", live_ascii_args)
        .replace("{live_ascii_bin}", &escape_kdl_path(live_ascii_bin))
        .replace("{chobits_bar_bin}", &escape_kdl_path(chobits_bar_bin))
        .replace("{chobits_send_bin}", &escape_kdl_path(chobits_send_bin))
        .replace("{zellij_bin}", &escape_kdl_path(zellij_bin))
}

pub fn build_layout_kdl(
    chobits_bin: &Path,
    plugin_wasm: &Path,
    interval_secs: u64,
    live_ascii_args: &str,
    live_ascii_bin: &Path,
    chobits_bar_bin: &Path,
    chobits_send_bin: &Path,
    zellij_bin: &Path,
) -> String {
    build_layout_kdl_from(
        DEFAULT_LAYOUT_KDL,
        chobits_bin,
        plugin_wasm,
        interval_secs,
        live_ascii_args,
        live_ascii_bin,
        chobits_bar_bin,
        chobits_send_bin,
        zellij_bin,
    )
}

/// Build the `args` line for live-ascii from config.
/// Generates individual tokens for the KDL `args` list:
///   `"<model>" "--camera" "--mouse" "--physics" "--image-protocol" "<protocol>"`
/// plus optional `--bg-color`, `--scale`, `--offsetx`, `--offsety`
#[allow(dead_code)]
pub fn live_ascii_args(cfg: &LiveAsciiConfig) -> String {
    let escaped_model = escape_kdl_path(&cfg.model_set);
    let mut parts = vec![format!("\"{}\"", escaped_model)];
    if cfg.enable_osf {
        parts.push("\"--camera\"".into());
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
/// (`chobits`, `chobits-bar`, `chobits-send`, `live-ascii`, `zellij`, ...)
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
        let with_ext = dir.join(name).with_extension(std::env::consts::EXE_EXTENSION);
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
/// anchor for all other paths (`config.toml`, `expressions/`, `models/`,
/// `layout.kdl`, `local/bin/`, `logs/`).
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

/// `<chobits-root>/expressions/`
pub fn expressions_dir() -> PathBuf {
    chobits_dir().join("expressions")
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

/// Resolve a path coming from `config.toml` (e.g. `[expressions] dir`,
/// `[live-ascii] model_set`):
/// - `~` / `~/...`   → expanded against the home directory
/// - absolute path   → used as-is
/// - relative path   → resolved against `<chobits-root>/`, *not* the
///   process's current working directory, since launching the exe (e.g. by
///   double-click) can leave CWD pointed at `bin/` rather than the root.
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
        // Resolve dir/model paths relative to <chobits-root>, expanding `~` too.
        config.expressions.dir = resolve_config_path(&config.expressions.dir);
        config.live_ascii.model_set = resolve_config_path(&config.live_ascii.model_set);
        config.zellij.config_dir = resolve_config_path(&config.zellij.config_dir);
        config.zellij.data_dir   = resolve_config_path(&config.zellij.data_dir);
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
            expressions: ExpressionsConfig {
                dir: default_expressions_dir(),
                idle_timeout_secs: default_idle_timeout(),
                osf_port: default_osf_port(),
            },
            zellij: ZellijConfig {
                config_dir: default_zellij_config_dir(),
                data_dir:   default_zellij_data_dir(),
                layout: default_layout(),
            },
            live_ascii: LiveAsciiConfig {
                model_set: default_live_ascii_model(),
                enable_osf: default_true(),
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
                    let close = if needle.contains('"') { "\"\"\"" } else { "'''" };
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
