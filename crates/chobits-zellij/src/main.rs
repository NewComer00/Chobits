use chobits_meta::snapshot::{truncate_snapshot, DEFAULT_SNAPSHOT_PORT};
use chobits_meta::viewport::{active_line_from_viewport, pane_screen_text as viewport_text};
use std::collections::BTreeMap;
use zellij_tile::prelude::*;

const PLUGIN_PERMISSIONS: &[PermissionType] =
    include!(concat!(env!("OUT_DIR"), "/plugin_permissions.rs"));

const DEFAULT_MAX_BYTES: usize = 4096;

struct State {
    manifest: PaneManifest,
    snapshot_port: u16,
    interval_secs: f64,
    max_bytes: usize,
    detached: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            manifest: PaneManifest::default(),
            snapshot_port: DEFAULT_SNAPSHOT_PORT,
            interval_secs: 10.0,
            max_bytes: DEFAULT_MAX_BYTES,
            detached: false,
        }
    }
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.snapshot_port = configuration
            .get("snapshot_port")
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_SNAPSHOT_PORT);

        self.interval_secs = configuration
            .get("interval_secs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10.0);

        self.max_bytes = configuration
            .get("max_bytes")
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_BYTES);

        request_permission(PLUGIN_PERMISSIONS);

        subscribe(&[
            EventType::PaneUpdate,
            EventType::Timer,
            EventType::SessionUpdate,
        ]);

        set_timeout(self.interval_secs);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PaneUpdate(manifest) => {
                self.manifest = manifest;
                false
            }
            Event::SessionUpdate(sessions, _) => {
                self.detached = sessions
                    .iter()
                    .find(|s| s.is_current_session)
                    .map(|s| s.connected_clients == 0)
                    .unwrap_or(false);
                false
            }
            Event::Timer(_) => {
                if !self.detached {
                    self.poll_focused_pane();
                }
                set_timeout(self.interval_secs);
                false
            }
            _ => false,
        }
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}
}

impl State {
    fn poll_focused_pane(&self) {
        let Some((tab_idx, pane)) = self.manifest.panes.iter().find_map(|(tab_idx, panes)| {
            panes
                .iter()
                .find(|p| p.is_focused && !p.is_plugin)
                .map(|p| (tab_idx, p))
        }) else {
            return;
        };

        let pane_id = PaneId::Terminal(pane.id);
        let contents = match get_pane_scrollback(pane_id, false) {
            Ok(contents) => contents,
            Err(_) => return,
        };

        let screen = viewport_text(&contents.viewport);
        if screen.is_empty() {
            return;
        }

        let cmd_str = match get_pane_running_command(pane_id) {
            Ok(args) if !args.is_empty() => args.join(" "),
            _ => pane
                .terminal_command
                .clone()
                .unwrap_or_else(|| pane.title.clone()),
        };

        // Manifest cursor can lag behind scrollback; query pane at poll time.
        let pane_info = get_pane_info(pane_id);
        let pane_for_cursor = pane_info.as_ref().unwrap_or(pane);
        let active_line = active_line_from_pane(pane_for_cursor, &contents.viewport);
        let snapshot = truncate_snapshot(
            &serde_json::json!({
                "tab": tab_idx.to_string(),
                "cmd": cmd_str,
                "active_line": active_line,
                "screen": screen,
            })
            .to_string(),
            self.max_bytes,
        );

        let url = format!("http://127.0.0.1:{}/snapshot", self.snapshot_port);
        let mut headers = BTreeMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let mut ctx = BTreeMap::new();
        ctx.insert("type".to_string(), "snapshot".to_string());
        web_request(url, HttpVerb::Post, headers, snapshot.into_bytes(), ctx);
    }
}

fn active_line_from_pane(pane: &PaneInfo, viewport: &[String]) -> String {
    let Some((_col, cursor_y)) = pane.cursor_coordinates_in_pane else {
        return String::new();
    };
    active_line_from_viewport(viewport, cursor_y, pane.pane_y, pane.pane_content_y)
}
