#![allow(dead_code)]
//! Utilities for integrating with Zellij
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::ExitStatus;

pub struct ZellijRunner {
    pub bin: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl ZellijRunner {
    pub fn new(bin: PathBuf, config_dir: PathBuf, data_dir: PathBuf) -> Self {
        Self {
            bin,
            config_dir,
            data_dir,
        }
    }

    fn base_cmd(&self) -> Command {
        let mut cmd = Command::new(&self.bin);
        cmd.args([
            "--config-dir",
            &self.config_dir.to_string_lossy(),
            "--data-dir",
            &self.data_dir.to_string_lossy(),
        ]);
        cmd
    }

    pub fn passthrough(&self, args: &[String]) -> i32 {
        self.base_cmd()
            .args(args)
            .status()
            .map(|s| s.code().unwrap_or(0))
            .unwrap_or_else(|e| {
                eprintln!("[start] Failed to run zellij: {e}");
                1
            })
    }

    pub fn list_sessions(&self) -> std::io::Result<Vec<String>> {
        let output = self
            .base_cmd()
            .args(["list-sessions", "--no-formatting"])
            .output()?;

        if !output.status.success() {
            let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(std::io::Error::other(if msg.is_empty() {
                format!("zellij list-sessions exited with {}", output.status)
            } else {
                format!("zellij list-sessions failed: {msg}")
            }));
        }

        Ok(filter_chobits_sessions(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }

    pub fn new_session(
        &self,
        session_name: &str,
        layout_path: &Path,
    ) -> std::io::Result<ExitStatus> {
        self.base_cmd()
            .args([
                "--new-session-with-layout",
                &layout_path.to_string_lossy(),
                "--session",
                session_name,
            ])
            .status()
    }

    pub fn attach(&self, session_name: &str) -> std::io::Result<ExitStatus> {
        self.base_cmd().args(["attach", session_name]).status()
    }
}

/// Zellij's cache directory, built directly per-platform (not via
/// `ProjectDirs::from`, since the qualifier/org/app triple Zellij itself
/// uses doesn't reproduce identically across platforms through the
/// `directories` crate — confirmed empirically: real Zellij on Windows
/// uses `%LOCALAPPDATA%\Zellij\cache`, not `...\Zellij Contributors\Zellij\cache`).
///
/// Zellij does not expose a cache-dir flag (`--data-dir` is separate), so
/// plugin permissions must be written here even when Chobits uses isolated
/// config/data directories.
pub fn zellij_cache_dir() -> Option<PathBuf> {
    let base = directories::BaseDirs::new()?;
    let cache_root = base.cache_dir(); // ~/.cache (Linux), ~/Library/Caches (macOS), %LOCALAPPDATA% (Windows)

    #[cfg(target_os = "windows")]
    {
        Some(cache_root.join("Zellij").join("cache"))
    }
    #[cfg(target_os = "macos")]
    {
        Some(cache_root.join("org.Zellij-Contributors.Zellij"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Some(cache_root.join("zellij"))
    }
}

/// `<zellij_cache_dir>/permissions.kdl`
pub fn zellij_permissions_kdl() -> Option<PathBuf> {
    zellij_cache_dir().map(|d| d.join("permissions.kdl"))
}

/// Pre-grant `permissions` to the plugin at `wasm_path`, skipping Zellij's
/// permission dialog. Writes/updates Zellij's `permissions.kdl` cache file
/// (creating it and its parent dir if missing). If an entry for this exact
/// path already exists, its permission list is replaced.
pub fn grant_plugin_permission(wasm_path: &Path, permissions: &[&str]) -> std::io::Result<()> {
    let kdl_path = zellij_permissions_kdl()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no home dir"))?;

    if let Some(parent) = kdl_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let existing = std::fs::read_to_string(&kdl_path).unwrap_or_default();
    let key = kdl_escape(&wasm_path.to_string_lossy());
    let mut entries = parse_permissions_kdl(&existing);

    match entries.iter_mut().find(|(k, _)| k == &key) {
        Some(entry) => entry.1 = permissions.iter().map(|p| p.to_string()).collect(),
        None => entries.push((key, permissions.iter().map(|p| p.to_string()).collect())),
    }

    std::fs::write(&kdl_path, render_permissions_kdl(&entries))
}

fn kdl_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn filter_chobits_sessions(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let name = line.split_whitespace().next()?.to_string();
            if name.starts_with("chobits-") && !line.contains("EXITED") {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

fn parse_permissions_kdl(content: &str) -> Vec<(String, Vec<String>)> {
    let mut entries = Vec::new();
    let mut lines = content.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if !trimmed.starts_with('"') || !trimmed.ends_with('{') {
            continue;
        }
        let Some(end_quote) = trimmed.rfind('"') else {
            continue;
        };
        if end_quote == 0 {
            continue;
        }
        let key = trimmed[1..end_quote].to_string();

        let mut perms = Vec::new();
        for inner in lines.by_ref() {
            let inner_trimmed = inner.trim();
            if inner_trimmed == "}" {
                break;
            }
            if !inner_trimmed.is_empty() {
                perms.push(inner_trimmed.to_string());
            }
        }
        entries.push((key, perms));
    }

    entries
}

fn render_permissions_kdl(entries: &[(String, Vec<String>)]) -> String {
    let mut out = String::new();
    for (key, perms) in entries {
        out.push_str(&format!("\"{}\" {{\n", key));
        for perm in perms {
            out.push_str("    ");
            out.push_str(perm);
            out.push('\n');
        }
        out.push_str("}\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kdl_escape_backslash_and_quote() {
        assert_eq!(kdl_escape(r#"C:\foo"bar"#), r##"C:\\foo\"bar"##);
    }

    #[test]
    fn parse_permissions_kdl_reads_entries() {
        let kdl = r##""/plugins/a.wasm" {
    ReadApplicationState
    RunCommands
}
"/other/b.wasm" {
    ReadPaneContents
}
"##;
        let entries = parse_permissions_kdl(kdl);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].0, "/plugins/a.wasm");
        assert_eq!(entries[0].1, vec!["ReadApplicationState", "RunCommands"]);
        assert_eq!(entries[1].0, "/other/b.wasm");
        assert_eq!(entries[1].1, vec!["ReadPaneContents"]);
    }

    #[test]
    fn render_permissions_kdl_round_trip() {
        let entries = vec![
            (
                "/plugins/a.wasm".to_string(),
                vec![
                    "ReadApplicationState".to_string(),
                    "RunCommands".to_string(),
                ],
            ),
            (
                "/other/b.wasm".to_string(),
                vec!["ReadPaneContents".to_string()],
            ),
        ];
        let rendered = render_permissions_kdl(&entries);
        assert_eq!(parse_permissions_kdl(&rendered), entries);
    }

    #[test]
    fn merge_permission_entry_replaces_existing() {
        let existing = r##""/plugins/a.wasm" {
    ReadApplicationState
}
"##;
        let key = kdl_escape("/plugins/a.wasm");
        let mut entries = parse_permissions_kdl(existing);
        let new_perms = ["ReadApplicationState", "ReadPaneContents", "WebAccess"];

        match entries.iter_mut().find(|(k, _)| k == &key) {
            Some(entry) => entry.1 = new_perms.iter().map(|p| (*p).to_string()).collect(),
            None => entries.push((key, new_perms.iter().map(|p| (*p).to_string()).collect())),
        }

        let reparsed = parse_permissions_kdl(&render_permissions_kdl(&entries));
        assert_eq!(reparsed.len(), 1);
        assert_eq!(reparsed[0].1, new_perms);
    }

    #[test]
    fn filter_chobits_sessions_skips_exited_and_foreign() {
        let text =
            "chobits-20250101-120000\nother-session\nchobits-old EXITED\nchobits-live attached";
        assert_eq!(
            filter_chobits_sessions(text),
            vec![
                "chobits-20250101-120000".to_string(),
                "chobits-live".to_string(),
            ]
        );
    }
}
