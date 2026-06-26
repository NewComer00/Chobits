use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::collections::VecDeque;
use std::time::Instant;

pub struct Message {
    pub text: String,
    pub timestamp: Instant,
}

pub struct AppState {
    pub messages: VecDeque<Message>,
    /// Lines scrolled up from the default bottom-aligned view.
    pub scroll_offset: u16,
    history_length: usize,
}

impl AppState {
    pub fn new(history_length: usize) -> Self {
        AppState {
            messages: VecDeque::with_capacity(history_length),
            scroll_offset: 0,
            history_length,
        }
    }

    pub fn push(&mut self, text: String, viewport_height: u16, width: usize) {
        let at_bottom = self.scroll_offset == 0;

        if self.messages.len() >= self.history_length {
            if let Some(front) = self.messages.pop_front() {
                if !at_bottom {
                    let removed = message_height(&front.text, width);
                    self.scroll_offset = self.scroll_offset.saturating_sub(removed);
                }
            }
        }

        let new_height = message_height(&text, width);
        self.messages.push_back(Message {
            text,
            timestamp: Instant::now(),
        });

        if at_bottom {
            self.scroll_offset = 0;
        } else {
            self.scroll_offset = self.scroll_offset.saturating_add(new_height);
            let max = max_scroll_offset(self, viewport_height, width);
            self.scroll_offset = self.scroll_offset.min(max);
        }
    }

    pub fn scroll_up(&mut self, lines: u16, max: u16) {
        self.scroll_offset = (self.scroll_offset + lines).min(max);
    }

    pub fn scroll_down(&mut self, lines: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }
}

fn message_height(text: &str, width: usize) -> u16 {
    let width = width.max(1);
    let wrap_lines: usize = text.split('\n').map(|l| l.len().div_ceil(width)).sum();
    wrap_lines.max(1) as u16
}

pub fn max_scroll_offset(state: &AppState, viewport_height: u16, width: usize) -> u16 {
    let total: u16 = state
        .messages
        .iter()
        .map(|m| message_height(&m.text, width))
        .sum();
    total.saturating_sub(viewport_height)
}

pub fn draw(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    let block = Block::new()
        .borders(Borders::ALL)
        .title("✦ chi")
        .style(Style::new().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let width = inner.width.max(1) as usize;
    let available = inner.height;

    let messages: Vec<&Message> = state.messages.iter().collect();
    let heights: Vec<u16> = messages
        .iter()
        .map(|msg| message_height(&msg.text, width))
        .collect();

    let total: u16 = heights.iter().sum();
    let max_scroll = total.saturating_sub(available);
    let scroll_top = max_scroll.saturating_sub(state.scroll_offset.min(max_scroll));

    let now = Instant::now();
    let msg_count = messages.len();
    let mut y = 0u16;

    for (i, msg) in messages.iter().enumerate() {
        let h = heights[i];
        let msg_bottom = y.saturating_add(h);

        if msg_bottom > scroll_top && y < scroll_top.saturating_add(available) {
            let skip_lines = scroll_top.saturating_sub(y);
            let visible_top = y.max(scroll_top);
            let visible_bottom = msg_bottom.min(scroll_top.saturating_add(available));
            let visible_height = visible_bottom.saturating_sub(visible_top);

            let age = now.duration_since(msg.timestamp).as_secs_f64();
            let position = msg_count - 1 - i;

            let rect = Rect {
                x: inner.x,
                y: inner.y + visible_top.saturating_sub(scroll_top),
                width: inner.width,
                height: visible_height,
            };

            let p = Paragraph::new(Line::from(vec![Span::styled(
                &msg.text,
                Style::new().fg(fade(position, age)),
            )]))
            .scroll((skip_lines, 0))
            .wrap(Wrap { trim: false });

            frame.render_widget(p, rect);
        }

        y = msg_bottom;
    }
}

/// Compute text color with fading based on position in ring buffer and age.
/// Newest (position=0) stays bright; older messages dim by position and age.
fn fade(position: usize, age_secs: f64) -> Color {
    let pos_factor = match position {
        0 => 1.0,
        1 => 0.75,
        2 => 0.55,
        3 => 0.40,
        _ => 0.30,
    };

    let time_factor = if position == 0 {
        1.0
    } else if age_secs > 30.0 {
        0.6
    } else if age_secs > 15.0 {
        0.8
    } else {
        1.0
    };

    let factor = pos_factor * time_factor;
    let v = (255.0 * factor) as u8;
    Color::Rgb(v, v, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_height_counts_wrapped_lines() {
        assert_eq!(message_height("hello", 10), 1);
        assert_eq!(message_height("12345678901", 10), 2);
        assert_eq!(message_height("a\nbbb", 2), 3);
    }

    #[test]
    fn max_scroll_offset_zero_when_content_fits() {
        let mut state = AppState::new(10);
        state.messages.push_back(Message {
            text: "hi".into(),
            timestamp: Instant::now(),
        });
        assert_eq!(max_scroll_offset(&state, 5, 20), 0);
    }

    #[test]
    fn push_at_bottom_keeps_scroll_at_zero() {
        let mut state = AppState::new(10);
        state.push("first".into(), 5, 20);
        state.push("second".into(), 5, 20);
        assert_eq!(state.scroll_offset, 0);
        assert_eq!(state.messages.len(), 2);
    }

    #[test]
    fn push_while_scrolled_up_increases_offset() {
        let mut state = AppState::new(10);
        state.push("first".into(), 2, 5);
        state.scroll_up(1, 10);
        assert!(state.scroll_offset > 0);
        let before = state.scroll_offset;
        state.push("second".into(), 2, 5);
        assert!(state.scroll_offset >= before);
    }

    #[test]
    fn push_evicts_oldest_at_history_limit() {
        let mut state = AppState::new(2);
        state.push("one".into(), 5, 20);
        state.push("two".into(), 5, 20);
        state.push("three".into(), 5, 20);
        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages.front().unwrap().text, "two");
        assert_eq!(state.messages.back().unwrap().text, "three");
    }

    #[test]
    fn scroll_up_and_down_clamp() {
        let mut state = AppState::new(10);
        state.push("line".into(), 2, 5);
        state.scroll_up(99, 1);
        assert_eq!(state.scroll_offset, 1);
        state.scroll_down(99);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn fade_newest_is_brightest() {
        match (fade(0, 0.0), fade(3, 0.0)) {
            (Color::Rgb(n0, _, _), Color::Rgb(n3, _, _)) => assert!(n0 > n3),
            _ => panic!("expected Rgb colors"),
        }
    }

    #[test]
    fn fade_older_messages_dim_with_age() {
        match (fade(1, 10.0), fade(1, 40.0)) {
            (Color::Rgb(young, _, _), Color::Rgb(old, _, _)) => assert!(young > old),
            _ => panic!("expected Rgb colors"),
        }
    }
}
