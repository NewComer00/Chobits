use std::collections::BTreeMap;
use zellij_tile::prelude::*;

const PLUGIN_PERMISSIONS: &[PermissionType] = include!(
    concat!(env!("OUT_DIR"), "/plugin_permissions.rs")
);

struct State {
    manifest: PaneManifest,
    chobits_send_bin: String,
    zellij_bin: String,
    interval_secs: f64,
}

impl Default for State {
    fn default() -> Self {
        State {
            manifest: PaneManifest::default(),
            chobits_send_bin: "chobits-send".into(),
            zellij_bin: "zellij".into(),
            interval_secs: 10.0,
        }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.chobits_send_bin = configuration
            .get("chobits_send_bin")
            .cloned()
            .unwrap_or_else(|| "chobits-send".into());

        self.zellij_bin = configuration
            .get("zellij_bin")
            .cloned()
            .unwrap_or_else(|| "zellij".into());

        self.interval_secs = configuration
            .get("interval_secs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10.0);

        request_permission(PLUGIN_PERMISSIONS);

        subscribe(&[
            EventType::PaneUpdate,
            EventType::Timer,
            EventType::RunCommandResult,
        ]);

        set_timeout(self.interval_secs);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PaneUpdate(manifest) => {
                self.manifest = manifest;
                false
            }
            Event::Timer(_) => {
                self.poll_focused_pane();
                set_timeout(self.interval_secs);
                false
            }
            Event::RunCommandResult(_exit_code, stdout, _stderr, context) => {
                if context.get("type").map(|s| s.as_str()) == Some("screen") {
                    let raw = String::from_utf8_lossy(&stdout).to_string();
                    let content = strip_ansi(&raw).trim().to_string();
                    if !content.is_empty() {
                        let tab = context.get("tab").cloned().unwrap_or_default();
                        let cmd = context.get("cmd").cloned().unwrap_or_default();
                        let snapshot = format!(
                            "{{\"tab\":{},\"cmd\":{},\"screen\":{}}}",
                            serde_json::to_string(&tab).unwrap(),
                            serde_json::to_string(&cmd).unwrap(),
                            serde_json::to_string(&content).unwrap(),
                        );
                        let mut ctx = BTreeMap::new();
                        ctx.insert("type".to_string(), "snapshot".to_string());
                        run_command(&[&self.chobits_send_bin, "--text", &snapshot], ctx);
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}
}

impl State {
    fn poll_focused_pane(&self) {
        for (tab_idx, panes) in &self.manifest.panes {
            for pane in panes {
                if pane.is_focused && !pane.is_plugin {
                    let cmd_str = match get_pane_running_command(PaneId::Terminal(pane.id)) {
                        Ok(args) if !args.is_empty() => args.join(" "),
                        _ => pane.terminal_command
                                .clone()
                                .unwrap_or_else(|| pane.title.clone()),
                    };

                    let pane_id_str = format!("terminal_{}", pane.id);
                    let mut ctx = BTreeMap::new();
                    ctx.insert("type".to_string(), "screen".to_string());
                    ctx.insert("tab".to_string(), tab_idx.to_string());
                    ctx.insert("cmd".to_string(), cmd_str);
                    run_command(
                        &[&self.zellij_bin, "action", "dump-screen",
                          "--pane-id", &pane_id_str],
                        ctx,
                    );

                    return;
                }
            }
        }
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for ch in chars.by_ref() {
                        if ch.is_ascii_alphabetic() { break; }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}
