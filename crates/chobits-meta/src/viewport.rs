//! Pane viewport helpers shared by the Zellij plugin and host-side tests.
//!
//! Run tests:
//!   cargo test -p chobits-meta
//! Plugin compile check (WASM only, not `cargo test` on native):
//!   cargo check -p chobits-zellij --target wasm32-wasip1

/// Build visible pane text from viewport lines (ANSI stripped).
pub fn pane_screen_text(viewport: &[String]) -> String {
    viewport
        .iter()
        .map(|line| strip_ansi_escapes::strip_str(line).trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Map pane-relative cursor Y to a viewport row index.
pub fn viewport_row(cursor_y: usize, pane_y: usize, pane_content_y: usize) -> usize {
    let frame_top = pane_content_y.saturating_sub(pane_y);
    cursor_y.saturating_sub(frame_top)
}

/// Line under the cursor in the current viewport.
pub fn active_line_from_viewport(
    viewport: &[String],
    cursor_y: usize,
    pane_y: usize,
    pane_content_y: usize,
) -> String {
    viewport
        .get(viewport_row(cursor_y, pane_y, pane_content_y))
        .map(|line| strip_ansi_escapes::strip_str(line).trim_end().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_screen_text_strips_ansi_and_trims() {
        let viewport = vec!["\x1b[31mhello\x1b[0m ".to_string(), "world".to_string()];
        assert_eq!(pane_screen_text(&viewport), "hello\nworld");
    }

    #[test]
    fn viewport_row_accounts_for_frame_offset() {
        // cursor at pane row 5, content starts 2 rows below pane top → viewport row 3
        assert_eq!(viewport_row(5, 0, 2), 3);
    }

    #[test]
    fn active_line_from_viewport_reads_cursor_line() {
        let viewport = vec!["line0".into(), "line1".into(), "line2".into()];
        assert_eq!(active_line_from_viewport(&viewport, 3, 0, 1), "line2");
    }

    #[test]
    fn active_line_empty_when_cursor_missing_from_viewport() {
        let viewport = vec!["only".into()];
        assert_eq!(active_line_from_viewport(&viewport, 99, 0, 0), "");
    }
}
