use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

/// Accept incoming snapshot connections on the given TCP port.
/// Each connection sends one text payload then closes.
pub async fn listen(
    port: u16,
    max_bytes: usize,
    mut handler: impl FnMut(String),
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    println!("[snapshot] Listening on {} (max {} bytes)", addr, max_bytes);

    loop {
        let (mut stream, peer) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("[snapshot] Accept error: {}", e);
                continue;
            }
        };

        // read_to_end loops until EOF (sender closes connection).
        // A single read() call is not guaranteed to return the full payload.
        let mut buf = Vec::with_capacity(max_bytes + 1024);
        match stream.read_to_end(&mut buf).await {
            Ok(0) => continue,
            Ok(_) => {}
            Err(e) => {
                eprintln!("[snapshot] Read error from {}: {}", peer, e);
                continue;
            }
        }

        let raw = String::from_utf8_lossy(&buf).trim().to_string();
        if raw.is_empty() {
            continue;
        }

        let original_len = raw.len();
        let text = truncate_snapshot(&raw, max_bytes);

        if original_len > max_bytes {
            println!(
                "[snapshot] Truncated {} → {} bytes",
                original_len,
                text.len()
            );
        }

        handler(text);
    }
}

/// Truncate snapshot to at most `max_bytes` bytes.
///
/// Tries to keep whole lines from the head and tail so both early and recent
/// context survive.  If the content has very long lines (e.g. a single wrapped
/// terminal line), falls back to a hard byte split at a UTF-8 character
/// boundary so the budget is always reasonably filled.
fn truncate_snapshot(raw: &str, max_bytes: usize) -> String {
    if raw.len() <= max_bytes {
        return raw.to_string();
    }

    let half = max_bytes / 2;
    let head = take_head(raw, half);
    let tail = take_tail(raw, half);

    format!(
        "{}\n\n... [{} bytes truncated] ...\n\n{}",
        head,
        raw.len() - max_bytes,
        tail
    )
}

/// Take up to `budget` bytes from the start, preferring whole lines.
/// Falls back to a hard byte slice if no line boundary fits.
fn take_head(s: &str, budget: usize) -> &str {
    if s.len() <= budget {
        return s;
    }

    // Walk lines, stop before exceeding budget.
    let mut end = 0;
    for line in s.lines() {
        let next = end + line.len() + 1; // +1 for '\n'
        if next > budget {
            break;
        }
        end = next;
    }

    if end > 0 {
        // Trim the trailing newline we counted but didn't verify is there.
        s[..end].trim_end_matches('\n')
    } else {
        // Single line longer than budget: hard slice at char boundary.
        let boundary = floor_char_boundary(s, budget);
        &s[..boundary]
    }
}

/// Take up to `budget` bytes from the end, preferring whole lines.
/// Falls back to a hard byte slice if no line boundary fits.
fn take_tail(s: &str, budget: usize) -> &str {
    if s.len() <= budget {
        return s;
    }

    let mut start = s.len();
    for line in s.lines().rev() {
        let prev = start.saturating_sub(line.len() + 1);
        if s.len() - prev > budget {
            break;
        }
        start = prev;
    }

    if start < s.len() {
        s[start..].trim_start_matches('\n')
    } else {
        // Single line longer than budget: hard slice at char boundary.
        let from = s.len() - budget;
        let boundary = ceil_char_boundary(s, from);
        &s[boundary..]
    }
}

/// Largest char boundary ≤ index.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    let index = index.min(s.len());
    (0..=index).rev().find(|&i| s.is_char_boundary(i)).unwrap_or(0)
}

/// Smallest char boundary ≥ index.
fn ceil_char_boundary(s: &str, index: usize) -> usize {
    let index = index.min(s.len());
    (index..=s.len()).find(|&i| s.is_char_boundary(i)).unwrap_or(s.len())
}
