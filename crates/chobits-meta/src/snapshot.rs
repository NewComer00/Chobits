/// Localhost HTTP port for `POST /snapshot` from the Zellij plugin (default **7880**).
pub const DEFAULT_SNAPSHOT_PORT: u16 = 7880;

/// Truncate snapshot to at most `max_bytes` bytes.
///
/// Tries to keep whole lines from the head and tail so both early and recent
/// context survive.  If the content has very long lines (e.g. a single wrapped
/// terminal line), falls back to a hard byte split at a UTF-8 character
/// boundary so the budget is always reasonably filled.
pub fn truncate_snapshot(raw: &str, max_bytes: usize) -> String {
    if raw.len() <= max_bytes {
        return raw.to_string();
    }

    let truncated_count = raw.len() - max_bytes;
    let marker = format!("\n\n... [{} bytes truncated] ...\n\n", truncated_count);
    let content_budget = max_bytes.saturating_sub(marker.len());
    if content_budget == 0 {
        let boundary = floor_char_boundary(raw, max_bytes);
        return raw[..boundary].to_string();
    }

    let half = content_budget / 2;
    let head = take_head(raw, half);
    let tail = take_tail(raw, half);

    format!("{head}{marker}{tail}")
}

/// Byte offsets where each line begins (handles `\n` and `\r\n`).
fn line_starts(s: &str) -> Vec<usize> {
    if s.is_empty() {
        return vec![];
    }
    let mut starts = vec![0];
    for (i, c) in s.char_indices() {
        if c == '\n' {
            let next = i + c.len_utf8();
            if next < s.len() {
                starts.push(next);
            }
        }
    }
    starts
}

/// Take up to `budget` bytes from the start, preferring whole lines.
/// Falls back to a hard byte slice if no line boundary fits.
fn take_head(s: &str, budget: usize) -> &str {
    if s.len() <= budget {
        return s;
    }

    let starts = line_starts(s);
    let mut end = 0;
    for (idx, _) in starts.iter().enumerate() {
        let line_end = starts.get(idx + 1).copied().unwrap_or(s.len());
        if line_end > budget {
            break;
        }
        end = line_end;
    }

    if end > 0 {
        s[..end].trim_end_matches('\n')
    } else {
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

    let starts = line_starts(s);
    let mut best_start = s.len();
    for &start in &starts {
        if s.len() - start <= budget {
            best_start = best_start.min(start);
        }
    }

    if best_start < s.len() {
        s[best_start..].trim_start_matches('\n')
    } else {
        let from = s.len().saturating_sub(budget);
        let boundary = ceil_char_boundary(s, from);
        &s[boundary..]
    }
}

/// Largest char boundary ≤ index.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    let index = index.min(s.len());
    (0..=index)
        .rev()
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(0)
}

/// Smallest char boundary ≥ index.
fn ceil_char_boundary(s: &str, index: usize) -> usize {
    let index = index.min(s.len());
    (index..=s.len())
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_tail_last_line_without_trailing_newline() {
        let s = "line1\nline2\nline3";
        assert_eq!(take_tail(s, 5), "line3");
        assert_eq!(take_tail(s, 12), "line2\nline3");
    }

    #[test]
    fn take_head_respects_line_boundaries() {
        let s = "line1\nline2\nline3";
        assert_eq!(take_head(s, 6), "line1");
        assert_eq!(take_head(s, 12), "line1\nline2");
    }

    #[test]
    fn truncate_splits_utf8_safely() {
        let s = "é".repeat(100);
        let out = truncate_snapshot(&s, 20);
        assert!(out.is_char_boundary(out.len()));
        assert!(out.len() <= 20);
    }

    #[test]
    fn truncate_output_within_max_bytes() {
        let s = "x\n".repeat(5000);
        let max = 4096;
        let out = truncate_snapshot(&s, max);
        assert!(out.len() <= max);
        assert!(out.contains("bytes truncated"));
    }

    #[test]
    fn truncate_short_input_unchanged() {
        let s = "hello\nworld";
        assert_eq!(truncate_snapshot(s, 100), s);
    }
}
