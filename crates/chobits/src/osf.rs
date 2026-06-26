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
//! use chobits::osf::OsfPlayer;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempExpressionsDir(PathBuf);

    impl TempExpressionsDir {
        fn new() -> Self {
            let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "chobits-osf-test-{}-{}",
                std::process::id(),
                id
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create temp expressions dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn write_expression(&self, name: &str, frame_count: usize) {
            let mut data = vec![0u8; FRAME_LEN * frame_count];
            for f in 0..frame_count {
                data[f * FRAME_LEN] = f as u8;
            }
            std::fs::write(self.0.join(format!("{name}.osf.bin")), data)
                .expect("write expression file");
        }
    }

    impl Drop for TempExpressionsDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn test_udp_pair() -> (Arc<UdpSocket>, UdpSocket, u16) {
        let recv = UdpSocket::bind("127.0.0.1:0").await.expect("bind recv");
        let port = recv.local_addr().expect("recv addr").port();
        let send = Arc::new(UdpSocket::bind("127.0.0.1:0").await.expect("bind send"));
        send.connect(format!("127.0.0.1:{port}"))
            .await
            .expect("connect send");
        (send, recv, port)
    }

    #[test]
    fn read_frames_parses_exact_chunks() {
        let dir = TempExpressionsDir::new();
        dir.write_expression("sample", 3);
        let path = dir.path().join("sample.osf.bin");
        let frames = OsfPlayer::read_frames(&path);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0][0], 0);
        assert_eq!(frames[2][0], 2);
    }

    #[test]
    fn read_frames_drops_trailing_partial_frame() {
        let dir = TempExpressionsDir::new();
        let path = dir.path().join("partial.osf.bin");
        let mut data = vec![0u8; FRAME_LEN + 10];
        data[0] = 42;
        std::fs::write(&path, data).unwrap();
        let frames = OsfPlayer::read_frames(&path);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0][0], 42);
    }

    #[test]
    fn new_errors_when_directory_missing() {
        let dir = std::env::temp_dir().join("chobits-osf-missing-dir-never-create-this");
        let rt = tokio::runtime::Runtime::new().unwrap();
        let sock = rt.block_on(async { Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()) });
        let result = OsfPlayer::new(&dir, "neutral", sock);
        assert!(result.is_err(), "expected missing directory error");
        let err = result.err().expect("error string");
        assert!(err.contains("does not exist"));
    }

    #[test]
    fn new_errors_when_fallback_missing() {
        let dir = TempExpressionsDir::new();
        dir.write_expression("happy", 1);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let sock = rt.block_on(async { Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()) });
        let result = OsfPlayer::new(dir.path(), "neutral", sock);
        assert!(result.is_err(), "expected missing fallback error");
        let err = result.err().expect("error string");
        assert!(err.contains("Fallback expression file does not exist"));
    }

    #[test]
    fn new_lists_expressions_sorted() {
        let dir = TempExpressionsDir::new();
        dir.write_expression("zebra", 1);
        dir.write_expression("alpha", 1);
        dir.write_expression("neutral", 1);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let sock = rt.block_on(async { Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap()) });
        let player = OsfPlayer::new(dir.path(), "neutral", sock).unwrap();
        assert_eq!(
            player.get_available_expressions(),
            vec!["alpha", "neutral", "zebra"]
        );
    }

    #[tokio::test]
    async fn play_sends_all_frames_once() {
        let dir = TempExpressionsDir::new();
        dir.write_expression("neutral", 1);
        dir.write_expression("happy", 2);

        let (send, recv, _) = test_udp_pair().await;
        let player = OsfPlayer::new(dir.path(), "neutral", send).unwrap();

        let (cancel_tx, cancel_rx) = oneshot::channel();
        let recv_task = tokio::spawn(async move {
            let mut buf = [0u8; FRAME_LEN];
            recv.recv(&mut buf).await.expect("frame 0");
            assert_eq!(buf[0], 0);
            recv.recv(&mut buf).await.expect("frame 1");
            assert_eq!(buf[0], 1);
            let _ = cancel_tx.send(());
        });

        player.play("happy", cancel_rx).await;
        recv_task.await.expect("recv task");
    }

    #[tokio::test]
    async fn play_falls_back_to_neutral_for_unknown_expression() {
        let dir = TempExpressionsDir::new();
        dir.write_expression("neutral", 1);

        let (send, recv, _) = test_udp_pair().await;
        let player = OsfPlayer::new(dir.path(), "neutral", send).unwrap();

        let (cancel_tx, cancel_rx) = oneshot::channel();
        let recv_task = tokio::spawn(async move {
            let mut buf = [0u8; FRAME_LEN];
            recv.recv(&mut buf).await.expect("neutral frame");
            assert_eq!(buf[0], 0);
            let _ = cancel_tx.send(());
        });

        player.play("missing", cancel_rx).await;
        recv_task.await.expect("recv task");
    }

    #[tokio::test]
    async fn play_looping_repeats_until_cancel() {
        let dir = TempExpressionsDir::new();
        dir.write_expression("neutral", 1);

        let (send, recv, _) = test_udp_pair().await;
        let player = OsfPlayer::new(dir.path(), "neutral", send).unwrap();

        let (cancel_tx, cancel_rx) = oneshot::channel();
        let recv_task = tokio::spawn(async move {
            let mut buf = [0u8; FRAME_LEN];
            for _ in 0..3 {
                recv.recv(&mut buf).await.expect("looped frame");
                assert_eq!(buf[0], 0);
            }
            let _ = cancel_tx.send(());
        });

        player.play_looping("neutral", cancel_rx).await;
        recv_task.await.expect("recv task");
    }
}
