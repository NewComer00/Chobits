use std::fs::{self, OpenOptions};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use chrono::Local;

use chobits_meta::PLUGIN_PERMISSIONS;

fn main() {
    println!("╔══════════════════════════════════════╗");
    println!("║       Chobits — Terminal Companion   ║");
    println!("╚══════════════════════════════════════╝");

    let config_dir = chobits::config::chobits_dir();
    let _ = fs::create_dir_all(&config_dir);

    let daemon_binary = chobits::config::find_executable("chobits");

    // Shared timestamp for this run's log file and zellij session name.
    let now = Local::now();
    let timestamp = now.format("%Y%m%d-%H%M%S").to_string();

    // 1. Spawn chobits daemon with stdout/stderr → log file
    let log_dir = chobits::config::log_dir();
    let _ = fs::create_dir_all(&log_dir);
    let log_path = log_dir.join(format!("chobits-{}.log", timestamp));
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .unwrap_or_else(|e| {
            eprintln!("[start] Failed to open log {:?}: {}", log_path, e);
            std::process::exit(1);
        });

    println!("[start] Launching chobits daemon (log: {:?})...", log_path);
    let mut daemon: Child = Command::new(&daemon_binary)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file))
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("[start] Failed to start chobits daemon: {}", e);
            eprintln!("[start] Looked for binary at: {:?}", daemon_binary);
            std::process::exit(1);
        });
    println!("[start] Daemon PID: {}", daemon.id());

    // 2. Wait for TCP :7878 to be ready
    let snapshot_port = 7878u16;
    let addr = format!("127.0.0.1:{}", snapshot_port);
    println!("[start] Waiting for daemon on {}...", addr);

    let max_attempts = 30;
    let mut connected = false;
    for attempt in 1..=max_attempts {
        match TcpStream::connect(&addr) {
            Ok(_) => {
                connected = true;
                println!("[start] Daemon ready after {} attempt(s)", attempt);
                break;
            }
            Err(_) => {
                if attempt < max_attempts {
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }
    }

    if !connected {
        eprintln!("[start] Daemon did not start within {} attempts", max_attempts);
        let _ = daemon.kill();
        std::process::exit(1);
    }

    // 3. Load config and build layout
    let config = chobits::Config::load().unwrap_or_else(|_| {
        eprintln!("[start] Warning: could not load config, using defaults");
        chobits::Config::default_config()
    });
    let config_path = chobits::config::config_path();
    let zellij_config_dir = config.zellij.config_dir;
    let zellij_data_dir = config.zellij.data_dir;
    let interval = config.snapshot.interval_secs;
    let live_ascii_args = chobits::config::live_ascii_args(&config.live_ascii);
    let wasm_path = find_wasm("chobits-zellij.wasm");
    let live_ascii_bin = chobits::config::find_executable("live-ascii");
    let chobits_bar_bin = chobits::config::find_executable("chobits-bar");
    let chobits_send_bin = chobits::config::find_executable("chobits-send");

    // Use user layout from config.toml if present, else fall back to embedded default.
    // Either way, substitute runtime templates (including bundled binary paths)
    // before writing, so the layout never depends on `$PATH`.
    let template = chobits::config::load_layout_from_config(&config_path);
    let final_kdl = chobits::config::build_layout_kdl_from(
        &template,
        &wasm_path,
        interval,
        &live_ascii_args,
        &live_ascii_bin,
        &chobits_bar_bin,
        &chobits_send_bin,
    );

    // 4. Create Zellij directories and write layout.kdl
    let zellij_layout_dir = zellij_config_dir.join("layouts");
    let zellij_plugin_dir = zellij_data_dir.join("plugins");

    for dir in [&zellij_config_dir, &zellij_data_dir, &zellij_layout_dir, &zellij_plugin_dir] {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("[start] Warning: could not create Zellij dir {:?}: {e}", dir);
        }
    }

    let layout_path = zellij_layout_dir.join("layout.kdl");
    if let Some(parent) = layout_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(e) = fs::write(&layout_path, &final_kdl) {
        eprintln!("[start] Failed to write layout to {:?}: {}", layout_path, e);
    }

    // 5. Pre-grant plugin permissions by writing to Zellij's cache file
    if let Err(e) = chobits::zellij::grant_plugin_permission(&wasm_path, PLUGIN_PERMISSIONS) {
        eprintln!("[start] Failed to grant Zellij plugin permissions: {}", e);
    }

    // 6. Launch zellij — prefer the bundled bin/local/zellij(.exe), fall back to $PATH
    let zellij_bin = chobits::config::find_executable("zellij");
    println!("[start] Launching zellij ({:?})...", zellij_bin);
    let session_name = format!("chobits-{}", timestamp);
    println!("[start] Session name: {session_name}");

    let zellij_status = Command::new(&zellij_bin)
        .args([
            "--config-dir",
            &zellij_config_dir.to_string_lossy(),
            "--data-dir",
            &zellij_data_dir.to_string_lossy(),
            "--new-session-with-layout",
            &layout_path.to_string_lossy(),
            "--session",
            &session_name,
        ])
        .spawn()
        .and_then(|mut child| child.wait())
        .map(|status| status.code());

    match zellij_status {
        Ok(Some(code)) if code != 0 => {
            eprintln!("[start] Zellij exited with code {}", code);
        }
        Err(e) => {
            eprintln!("[start] Failed to launch zellij: {}", e);
        }
        _ => {}
    }

    // 7. Cleanup — kill the zellij session first, then the daemon.
    // The session may already be gone (user exited normally), so ignore errors.
    println!("[start] Zellij closed, cleaning up...");
    let _ = Command::new(&zellij_bin)
        .args([
            "--config-dir", &zellij_config_dir.to_string_lossy(),
            "--data-dir",   &zellij_data_dir.to_string_lossy(),
            "delete-session", &session_name, "--force",
        ])
        .status();
    let _ = daemon.kill();
    println!("[start] Done.");
}

/// Locate `chobits-zellij.wasm`. Search order:
/// 1. `<chobits-root>/local/bin/` — packaged layout
/// 2. `<chobits-root>/bin/`       — fallback for unusual layouts
/// 3. Walk up from the exe to find a Cargo workspace's
///    `target/wasm32-wasip1/{debug,release}/` — useful for `cargo run`
fn find_wasm(name: &str) -> PathBuf {
    for dir in [chobits::config::local_bin_dir(), chobits::config::bin_dir()] {
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
