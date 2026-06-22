mod bar;
mod config;
mod llm;
mod osf;
mod snapshot;

use config::Config;
use osf::OsfPlayer;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot, Mutex};
use rand::seq::IndexedRandom;

static QUIET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct SnapshotEntry {
    content: String,
    timestamp: Instant,
}

const HISTORY_CAPACITY: usize = 16;

struct AppState {
    config: Config,
    osf_player: OsfPlayer,
    osf_cancel_tx: Mutex<Option<oneshot::Sender<()>>>,
    llm_busy: Mutex<bool>,
    history: Mutex<VecDeque<SnapshotEntry>>,
    in_monologue: Mutex<bool>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut quiet = false;
    while let Some(arg) = args.next() {
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
    if !quiet {
        println!("  snapshot port: {}", config.ports.snapshot);
        println!("  bar port:      {}", config.ports.bar);
        println!("  osf port:      {}", config.ports.osf);
        println!("  llm backend:   {} ({})", config.llm.backend, config.llm.model);
    }

    let _gag_stdout = if quiet { Some(gag::Gag::stdout()?) } else { None::<gag::Gag> };
    let _gag_stderr = if quiet { Some(gag::Gag::stderr()?) } else { None::<gag::Gag> };

    let osf_socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
    osf_socket.connect(format!("127.0.0.1:{}", config.ports.osf)).await?;

    let osf_player = OsfPlayer::new(config.expressions.dir.clone(), "neutral", osf_socket)?;

    let state = Arc::new(AppState {
        config: config.clone(),
        osf_player,
        osf_cancel_tx: Mutex::new(None),
        llm_busy: Mutex::new(false),
        history: Mutex::new(VecDeque::with_capacity(HISTORY_CAPACITY)),
        in_monologue: Mutex::new(false),
    });

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let listener_tx = tx.clone();
    let snapshot_port = config.ports.snapshot;
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

        // Drop early while the LLM is busy — this now covers the whole
        // "thinking" animation window too, so we never let a static-screen
        // monologue transition steal the osf_cancel_tx slot mid-thought.
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
            let changed = hist.back().map_or(true, |e| e.content != snapshot_text);

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
            std::time::Duration::from_secs(config.expressions.idle_timeout_secs);

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
            let mut cancel = state.osf_cancel_tx.lock().await;
            if let Some(tx) = cancel.take() {
                let _ = tx.send(());
            }
        }

        tokio::spawn(async move {
            // Start a "thinking" loop immediately — covers prompt-building
            // and the LLM call. If no "thinking.osf.bin" exists, OsfPlayer
            // falls back to "neutral" automatically.
            let (think_tx, think_rx) = oneshot::channel();
            {
                let mut cancel = state.osf_cancel_tx.lock().await;
                *cancel = Some(think_tx);
            }
            let thinking_player = state.osf_player.clone();
            tokio::spawn(async move {
                thinking_player.play_looping("thinking", think_rx).await;
            });

            let system_prompt = llm::build_system_prompt(
                &config.persona.name,
                &config.persona.description,
                &config.expressions.dir,
            );

            println!("[chobits] snapshot received: {:?}", snapshot_text);

            let prompt = format!(
                "{}\n\nUser's terminal snapshot:\n{}",
                system_prompt, snapshot_text
            );

            let backend = llm::Backend::from_config(&config.llm);
            // backend.query is a blocking call — run it off the async
            // executor so the thinking animation actually keeps ticking
            // while we wait on it.
            let response = tokio::task::spawn_blocking(move || backend.query(&prompt))
                .await
                .unwrap_or_else(|e| {
                    eprintln!("[chobits] LLM call panicked: {}", e);
                    None
                });

            let parsed = match response {
                Some(raw) => llm::parse_response(&raw).unwrap_or_else(|| llm::LlmResponse {
                    text: "hmm?".into(),
                    expression: "neutral".into(),
                }),
                None => llm::LlmResponse {
                    text: "...".into(),
                    expression: "neutral".into(),
                },
            };

            println!(
                "[chobits] LLM → text: {:?}, expression: {:?}",
                parsed.text, parsed.expression
            );

            *state.llm_busy.lock().await = false;

            let bar_text = parsed.text.clone();
            let bar_port = config.ports.bar;
            tokio::spawn(async move {
                if let Err(e) = bar::send_text(bar_port, &bar_text).await {
                    eprintln!("[chobits] Failed to send to bar: {}", e);
                }
            });

            // LLM responded — stop the thinking loop, play the chosen
            // expression once, then settle into looping neutral.
            let (cancel_tx, cancel_rx) = oneshot::channel();
            {
                let mut cancel = state.osf_cancel_tx.lock().await;
                if let Some(old_tx) = cancel.replace(cancel_tx) {
                    let _ = old_tx.send(());
                }
            }
            state.osf_player.play(&parsed.expression, cancel_rx).await;

            let (cancel_tx2, cancel_rx2) = oneshot::channel();
            {
                let mut cancel = state.osf_cancel_tx.lock().await;
                *cancel = Some(cancel_tx2);
            }
            state.osf_player.play_looping("neutral", cancel_rx2).await;
        });
    }

    Ok(())
}

async fn stop_monologue(state: &Arc<AppState>) {
    let mut in_mono = state.in_monologue.lock().await;
    if *in_mono {
        *in_mono = false;
        if let Some(tx) = state.osf_cancel_tx.lock().await.take() {
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

    let state = state.clone();
    tokio::spawn(async move {
        loop {
            if !play_monologue_step(&state, "neutral").await {
                break;
            }

            let expressions = state.osf_player.get_available_expressions();
            let name = match expressions.choose(&mut rand::rng()) {
                Some(n) => n.clone(),
                None => break,
            };
            if !play_monologue_step(&state, &name).await {
                break;
            }
        }
    });
}

/// Play one expression as a single step of the monologue loop, registering
/// its cancel sender in the shared osf_cancel_tx slot (the same slot used
/// by the normal dispatch path and stop_monologue). Returns whether the
/// monologue state is still active — false means the caller should stop
/// looping.
async fn play_monologue_step(state: &Arc<AppState>, name: &str) -> bool {
    if !*state.in_monologue.lock().await {
        return false; // already cancelled before this step even started
    }

    let (cancel_tx, cancel_rx) = oneshot::channel();
    {
        let mut cancel = state.osf_cancel_tx.lock().await;
        if let Some(old_tx) = cancel.replace(cancel_tx) {
            let _ = old_tx.send(());
        }
    }
    state.osf_player.play(name, cancel_rx).await;

    *state.in_monologue.lock().await
}
