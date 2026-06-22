//! OSF (Open Smart Face) expression player.
//!
//! Loads `.osf.bin` expression files and sends their frames
//! over UDP to a connected hardware device.
//!
//! All expression files are expected to reside in a single flat
//! directory provided at construction time. All expression frames
//! are read into memory once during construction and served from
//! an in-memory cache afterwards — `play`/`play_looping` never touch
//! disk. The UDP socket is supplied once during initialisation and
//! reused for all subsequent playbacks.
//!
//! # Example
//!
//! ```no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//! use tokio::net::UdpSocket;
//! use tokio::sync::oneshot;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let sock = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
//!     sock.connect("127.0.0.1:1234").await?;
//!
//!     let player = OsfPlayer::new("./expressions", "neutral", sock)?;
//!
//!     let (tx, rx) = oneshot::channel();
//!     tokio::spawn(async move {
//!         tokio::time::sleep(Duration::from_secs(2)).await;
//!         let _ = tx.send(());
//!     });
//!     player.play("happy", rx).await;
//!
//!     let (tx2, rx2) = oneshot::channel();
//!     tokio::spawn(async move {
//!         tokio::time::sleep(Duration::from_secs(5)).await;
//!         let _ = tx2.send(());
//!     });
//!     player.play_looping("neutral", rx2).await;
//!
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::oneshot;

pub const FRAME_LEN: usize = 1785;
pub const FRAME_INTERVAL: Duration = Duration::from_millis(40);

#[derive(Clone)]
pub struct OsfPlayer {
    fallback_expression: String,
    sock: Arc<UdpSocket>,
    cache: Arc<HashMap<String, Arc<Vec<[u8; FRAME_LEN]>>>>,
}

impl OsfPlayer {
    /// Create a new player. All `.osf.bin` files under `expressions_dir`
    /// are read into memory immediately; `play`/`play_looping` only ever
    /// hit this in-memory cache.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `expressions_dir` does not exist or is not a directory.
    /// - The fallback expression (`<fallback>.osf.bin`) is missing from
    ///   `expressions_dir`.
    pub fn new(
        expressions_dir: impl Into<std::path::PathBuf>,
        fallback_expression: impl Into<String>,
        sock: Arc<UdpSocket>,
    ) -> Result<Self, String> {
        let expressions_dir: std::path::PathBuf = expressions_dir.into();
        let fallback_expression: String = fallback_expression.into();

        if !expressions_dir.exists() {
            return Err(format!(
                "Expressions directory does not exist: {:?}",
                expressions_dir
            ));
        }
        if !expressions_dir.is_dir() {
            return Err(format!(
                "Expressions path is not a directory: {:?}",
                expressions_dir
            ));
        }

        let mut cache = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(&expressions_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let fname = match entry.file_name().into_string() {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let stem = match fname.strip_suffix(".osf.bin") {
                    Some(s) => s,
                    None => continue,
                };
                let frames = Self::read_frames(&path);
                cache.insert(stem.to_string(), Arc::new(frames));
            }
        }

        if !cache.contains_key(&fallback_expression) {
            return Err(format!(
                "Fallback expression file does not exist: {:?}",
                expressions_dir.join(format!("{}.osf.bin", fallback_expression))
            ));
        }

        Ok(Self {
            fallback_expression,
            sock,
            cache: Arc::new(cache),
        })
    }

    /// Read raw frames from a single expression file on disk.
    /// Only used internally during construction.
    fn read_frames(path: &Path) -> Vec<[u8; FRAME_LEN]> {
        let data = std::fs::read(path).unwrap_or_else(|e| {
            eprintln!("[osf] Failed to read expression file {:?}: {}", path, e);
            vec![]
        });
        data.chunks_exact(FRAME_LEN)
            .map(|c| c.try_into().unwrap())
            .collect()
    }

    /// Look up cached frames for a named expression, falling back if missing.
    fn load_named(&self, name: &str) -> Arc<Vec<[u8; FRAME_LEN]>> {
        if let Some(frames) = self.cache.get(name) {
            return frames.clone();
        }
        eprintln!(
            "[osf] Expression not in cache: {:?}, falling back to {:?}",
            name, self.fallback_expression
        );
        self.cache
            .get(&self.fallback_expression)
            .cloned()
            .unwrap_or_else(|| Arc::new(Vec::new()))
    }

    /// Return a sorted list of expressions currently loaded in the cache.
    pub fn get_available_expressions(&self) -> Vec<String> {
        let mut names: Vec<String> = self.cache.keys().cloned().collect();
        names.sort();
        names
    }

    /// Play an expression once, respecting cancellation.
    pub async fn play(&self, name: &str, cancel_rx: oneshot::Receiver<()>) {
        let frames = self.load_named(name);
        if frames.is_empty() {
            return;
        }
        self.send_frames(&frames, cancel_rx, false).await;
    }

    /// Play an expression in an infinite loop until cancelled.
    ///
    /// If the expression has no frames, the function blocks until
    /// `cancel_rx` fires instead of busy‑looping.
    pub async fn play_looping(&self, name: &str, cancel_rx: oneshot::Receiver<()>) {
        let frames = self.load_named(name);
        if frames.is_empty() {
            let _ = cancel_rx.await;
            return;
        }
        self.send_frames(&frames, cancel_rx, true).await;
    }

    async fn send_frames(
        &self,
        frames: &[[u8; FRAME_LEN]],
        mut cancel_rx: oneshot::Receiver<()>,
        loop_until_cancel: bool,
    ) {
        if frames.is_empty() {
            if loop_until_cancel {
                let _ = cancel_rx.await;
            }
            return;
        }

        loop {
            for frame in frames {
                tokio::select! {
                    _ = &mut cancel_rx => {
                        return;
                    }
                    _ = async {
                        if let Err(e) = self.sock.send(frame).await {
                            eprintln!("[osf] UDP send error: {}", e);
                        }
                        tokio::time::sleep(FRAME_INTERVAL).await;
                    } => {}
                }
            }
            if !loop_until_cancel {
                break;
            }
        }
    }
}
