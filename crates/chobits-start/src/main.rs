use std::fs;
use chrono::Local;

use chobits_meta::PLUGIN_PERMISSIONS;
use chobits::zellij::ZellijRunner;

fn main() {
    println!("╔══════════════════════════════════════╗");
    println!("║       Chobits — Terminal Companion   ║");
    println!("╚══════════════════════════════════════╝");

    let config_dir = chobits::config::chobits_dir();
    let _ = fs::create_dir_all(&config_dir);

    let now = Local::now();
    let timestamp = now.format("%Y%m%d-%H%M%S").to_string();

    // 1. Load config
    let config = chobits::Config::load().unwrap_or_else(|_| {
        eprintln!("[start] Warning: could not load config, using defaults");
        chobits::Config::default_config()
    });

    let zellij_bin = chobits::config::find_executable("zellij");
    let zellij_config_dir = config.zellij.config_dir.clone();
    let zellij_data_dir = config.zellij.data_dir.clone();
    let zellij = ZellijRunner::new(zellij_bin, zellij_config_dir.clone(), zellij_data_dir.clone());

    // Early exit for subcommands
    let mut args = std::env::args().skip(1);
    if let Some(cmd) = args.next() {
        match cmd.as_str() {
            "zellij" => {
                let rest: Vec<String> = args.collect();
                std::process::exit(zellij.passthrough(&rest));
            }
            "help" | "--help" | "-h" => {
                println!("Usage: chobits-start [COMMAND]");
                println!();
                println!("Commands:");
                println!("  zellij <args>   Pass arguments to the bundled Zellij instance");
                println!();
                println!("Examples:");
                println!("  chobits-start                  Launch or attach to a Chobits session");
                println!("  chobits-start zellij ls        List Zellij sessions");
                println!("  chobits-start zellij --help    Show Zellij help");
                std::process::exit(0);
            }
            other => {
                eprintln!("[start] Unknown command: {other}");
                eprintln!("Run 'chobits-start --help' for usage.");
                std::process::exit(1);
            }
        }
    }

    // 2. Build layout
    let config_path = chobits::config::config_path();
    let interval = config.snapshot.interval_secs;
    let live_ascii_args = chobits::config::live_ascii_args(&config.live_ascii);
    let chobits_bin = chobits::config::find_executable("chobits");
    let wasm_path = chobits::config::find_wasm("chobits-zellij.wasm");
    let live_ascii_bin = chobits::config::find_executable("live-ascii");
    let chobits_bar_bin = chobits::config::find_executable("chobits-bar");
    let chobits_send_bin = chobits::config::find_executable("chobits-send");

    let template = chobits::config::load_layout_from_config(&config_path);
    let final_kdl = chobits::config::build_layout_kdl_from(
        &template,
        &chobits_bin,
        &wasm_path,
        interval,
        &live_ascii_args,
        &live_ascii_bin,
        &chobits_bar_bin,
        &chobits_send_bin,
        &zellij.bin,
    );

    // 3. Create Zellij directories and write layout.kdl
    let zellij_layout_dir = zellij_config_dir.join("layouts");
    let zellij_plugin_dir = zellij_data_dir.join("plugins");

    for dir in [&zellij_config_dir, &zellij_data_dir, &zellij_layout_dir, &zellij_plugin_dir] {
        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!("[start] Warning: could not create Zellij dir {:?}: {e}", dir);
        }
    }

    let layout_path = zellij_layout_dir.join("layout.kdl");
    if let Err(e) = fs::write(&layout_path, &final_kdl) {
        eprintln!("[start] Failed to write layout to {:?}: {}", layout_path, e);
    }

    // 4. Pre-grant plugin permissions
    if let Err(e) = chobits::zellij::grant_plugin_permission(&wasm_path, PLUGIN_PERMISSIONS) {
        eprintln!("[start] Failed to grant Zellij plugin permissions: {}", e);
    }

    // 5. Attach to existing session or create a new one
    match zellij.list_sessions().as_slice() {
        [] => {
            let session_name = format!("chobits-{}", timestamp);
            println!("[start] Creating new session: {session_name}");
            match zellij.new_session(&session_name, &layout_path) {
                Ok(s) if !s.success() => eprintln!("[start] Zellij exited with code {:?}", s.code()),
                Err(e) => eprintln!("[start] Failed to launch zellij: {e}"),
                _ => {}
            }
        }
        [session_name] => {
            println!("[start] Attaching to existing session: {session_name}");
            match zellij.attach(session_name) {
                Ok(s) if !s.success() => eprintln!("[start] Zellij attach exited with code {:?}", s.code()),
                Err(e) => eprintln!("[start] Failed to attach: {e}"),
                _ => {}
            }
        }
        sessions => {
            println!("[start] Multiple chobits sessions running:");
            for (i, s) in sessions.iter().enumerate() {
                println!("  [{}] {}", i + 1, s);
            }
            print!("[start] Select session to attach (1-{}): ", sessions.len());
            std::io::Write::flush(&mut std::io::stdout()).ok();

            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();

            match input.trim().parse::<usize>().ok().filter(|&n| n >= 1 && n <= sessions.len()) {
                Some(n) => {
                    let session_name = &sessions[n - 1];
                    println!("[start] Attaching to: {session_name}");
                    let _ = zellij.attach(session_name);
                }
                None => {
                    eprintln!("[start] Invalid selection, aborting.");
                    std::process::exit(1);
                }
            }
        }
    }
}
