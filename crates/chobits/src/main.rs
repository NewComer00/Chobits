mod bar;
mod config;
mod llm;
mod snapshot;
mod vts;

use config::Config;
use rand::seq::IndexedRandom;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, oneshot, Mutex};
use vts::VtsPlayer;

static QUIET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct SnapshotEntry {
    content: String,
    timestamp: Instant,
}

const HISTORY_CAPACITY: usize = 16;

struct AppState {
    config: Config,
    vts_player: VtsPlayer,
    vts_cancel_tx: Mutex<Option<oneshot::Sender<()>>>,
    llm_busy: Mutex<bool>,
    history: Mutex<VecDeque<SnapshotEntry>>,
    in_monologue: Mutex<bool>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    chobits_meta::apply_loopback_no_proxy_to_process();

    let args = std::env::args().skip(1);
    let mut quiet = false;
    for arg in args {
        match arg.as_str() {
            "--quiet" | "-q" => quiet = true,
            _ => {}
        }
    }
    QUIET.store(quiet, std::sync::atomic::Ordering::Relaxed);

    if !quiet {
        println!("[chobits] Starting daemon with config:");
    }

    let config = Config::load()?;
    config::warn_vts_port_mismatch(&config.live_ascii, &config.vts);
    if !quiet {
        println!("  snapshot port: {}", config.snapshot.port);
        println!("  bar port:      {}", config.bar.port);
        println!("  vts url:       {}", config.vts.url);
        println!(
            "  llm backend:   {} ({})",
            config.llm.backend, config.llm.model
        );
    }

    let _gag_stdout = if quiet {
        Some(gag::Gag::stdout()?)
    } else {
        None::<gag::Gag>
    };
    let _gag_stderr = if quiet {
        Some(gag::Gag::stderr()?)
    } else {
        None::<gag::Gag>
    };

    let vts_player = VtsPlayer::new(&config.vts).await?;

    let state = Arc::new(AppState {
        config: config.clone(),
        vts_player,
        vts_cancel_tx: Mutex::new(None),
        llm_busy: Mutex::new(false),
        history: Mutex::new(VecDeque::with_capacity(HISTORY_CAPACITY)),
        in_monologue: Mutex::new(false),
    });

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let listener_tx = tx.clone();
    let snapshot_port = config.snapshot.port;
    tokio::spawn(async move {
        if let Err(e) = snapshot::listen(snapshot_port, config.snapshot.max_bytes, move |text| {
            let _ = listener_tx.send(text);
        })
        .await
        {
            eprintln!("[chobits] Snapshot listener error: {}", e);
        }
    });

    while let Some(snapshot_text) = rx.recv().await {
        let state = state.clone();
        let config = state.config.clone();

        {
            let busy = state.llm_busy.lock().await;
            if *busy {
                println!("[chobits] LLM busy, dropping snapshot");
                continue;
            }
        }

        let now = Instant::now();

        let (unchanged_for, screen_changed) = {
            let mut hist = state.history.lock().await;
            let changed = hist.back().is_none_or(|e| e.content != snapshot_text);

            hist.push_back(SnapshotEntry {
                content: snapshot_text.clone(),
                timestamp: now,
            });
            if hist.len() > HISTORY_CAPACITY {
                hist.pop_front();
            }

            let mut unchanged_since = now;
            for entry in hist.iter().rev() {
                if entry.content == snapshot_text {
                    unchanged_since = entry.timestamp;
                } else {
                    break;
                }
            }
            (now.duration_since(unchanged_since), changed)
        };

        let static_screen_threshold =
            std::time::Duration::from_secs(config.idle.idle_timeout_secs);

        if screen_changed {
            stop_monologue(&state).await;
        } else if unchanged_for >= static_screen_threshold {
            start_monologue_if_needed(&state).await;
            continue;
        } else {
            continue;
        }

        *state.llm_busy.lock().await = true;

        {
            let mut cancel = state.vts_cancel_tx.lock().await;
            if let Some(tx) = cancel.take() {
                let _ = tx.send(());
            }
        }

        tokio::spawn(async move {
            let default_loop_name = state.vts_player.default_loop_name();
            let default_label = state.vts_player.default_expression_label();
            let llm_aliases = state.vts_player.llm_aliases();

            let (think_tx, think_rx) = oneshot::channel();
            {
                let mut cancel = state.vts_cancel_tx.lock().await;
                *cancel = Some(think_tx);
            }
            let waiting_player = state.vts_player.clone();
            tokio::spawn(async move {
                waiting_player.play_looping("thinking", think_rx).await;
            });

            let system_prompt = llm::build_system_prompt(
                &config.persona.name,
                &config.persona.description,
                &llm_aliases,
                &default_label,
            );

            println!("[chobits] snapshot received: {:?}", snapshot_text);

            let prompt = format!(
                "{}\n\nUser's terminal snapshot:\n{}",
                system_prompt, snapshot_text
            );

            let backend = llm::Backend::from_config(&config.llm);
            let response = tokio::task::spawn_blocking(move || backend.query(&prompt))
                .await
                .unwrap_or_else(|e| {
                    eprintln!("[chobits] LLM call panicked: {}", e);
                    None
                });

            let parsed = match response {
                Some(raw) => llm::parse_response(&raw, &default_label)
                    .expect("parse_response always returns Some"),
                None => llm::LlmResponse {
                    text: "...".into(),
                    expression: default_label.clone(),
                },
            };

            println!(
                "[chobits] LLM → text: {:?}, expression: {:?}",
                parsed.text, parsed.expression
            );

            *state.llm_busy.lock().await = false;

            let bar_text = parsed.text.clone();
            let bar_port = config.bar.port;
            tokio::spawn(async move {
                if let Err(e) = bar::send_text(bar_port, &bar_text).await {
                    eprintln!("[chobits] Failed to send to bar: {}", e);
                }
            });

            let (cancel_tx, cancel_rx) = oneshot::channel();
            {
                let mut cancel = state.vts_cancel_tx.lock().await;
                if let Some(old_tx) = cancel.replace(cancel_tx) {
                    let _ = old_tx.send(());
                }
            }
            if parsed.expression != default_label {
                state.vts_player.play(&parsed.expression, cancel_rx).await;
            }

            let (cancel_tx2, cancel_rx2) = oneshot::channel();
            {
                let mut cancel = state.vts_cancel_tx.lock().await;
                *cancel = Some(cancel_tx2);
            }
            state.vts_player.play_looping(&default_loop_name, cancel_rx2).await;
        });
    }

    Ok(())
}

async fn stop_monologue(state: &Arc<AppState>) {
    let mut in_mono = state.in_monologue.lock().await;
    if *in_mono {
        *in_mono = false;
        if let Some(tx) = state.vts_cancel_tx.lock().await.take() {
            let _ = tx.send(());
        }
    }
}

async fn start_monologue_if_needed(state: &Arc<AppState>) {
    {
        let mut in_mono = state.in_monologue.lock().await;
        if *in_mono {
            return;
        }
        *in_mono = true;
    }

    if let Some(tx) = state.vts_cancel_tx.lock().await.take() {
        let _ = tx.send(());
    }

    let state = state.clone();
    tokio::spawn(async move {
        let default_label = state.vts_player.default_expression_label();
        loop {
            if !play_monologue_step(&state, &default_label).await {
                break;
            }

            let aliases: Vec<String> = state
                .vts_player
                .llm_aliases()
                .into_iter()
                .filter(|n| n != &default_label)
                .collect();
            let name = match aliases.choose(&mut rand::rng()) {
                Some(n) => n.clone(),
                None => break,
            };
            if !play_monologue_step(&state, &name).await {
                break;
            }
        }
    });
}

async fn play_monologue_step(state: &Arc<AppState>, name: &str) -> bool {
    if !*state.in_monologue.lock().await {
        return false;
    }

    let (cancel_tx, cancel_rx) = oneshot::channel();
    {
        let mut cancel = state.vts_cancel_tx.lock().await;
        if let Some(old_tx) = cancel.replace(cancel_tx) {
            let _ = old_tx.send(());
        }
    }
    state.vts_player.play(name, cancel_rx).await;

    *state.in_monologue.lock().await
}
