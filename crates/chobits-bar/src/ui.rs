use ratatui::{
    layout::{Constraint, Direction, Layout},
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
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            messages: VecDeque::with_capacity(5),
        }
    }

    pub fn push(&mut self, text: String) {
        if self.messages.len() >= 5 {
            self.messages.pop_front();
        }
        self.messages.push_back(Message {
            text,
            timestamp: Instant::now(),
        });
    }
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

    // Calculate heights for each message (newest last = bottom)
    let messages: Vec<&Message> = state.messages.iter().collect();
    let heights: Vec<Constraint> = messages
        .iter()
        .map(|msg| {
            // count actual wrap lines
            let lines = msg.text.chars().filter(|&c| c == '\n').count() + 1;
            let wrap_lines: usize = msg.text.split('\n').map(|l| {
                (l.len() + width - 1) / width
            }).sum();
            let total = wrap_lines.max(lines) as u16;
            Constraint::Length(total.min(inner.height))
        })
        .collect();

    // Check if total height exceeds inner — trim oldest from top
    let total: u16 = heights.iter().map(|c| match c {
        Constraint::Length(n) => *n,
        _ => 1,
    }).sum();

    // Only render as many messages as fit, keeping newest (last) ones
    let available = inner.height;
    let mut used = 0u16;
    let mut start_idx = messages.len();
    for i in (0..messages.len()).rev() {
        let h = match heights[i] { Constraint::Length(n) => n, _ => 1 };
        if used + h > available { break; }
        used += h;
        start_idx = i;
    }

    let visible_msgs = &messages[start_idx..];
    let visible_heights: Vec<Constraint> = heights[start_idx..].to_vec();

    let chunks = Layout::new(Direction::Vertical, visible_heights).split(inner);

    let now = Instant::now();
    let msg_count = visible_msgs.len();

    for (i, msg) in visible_msgs.iter().enumerate() {
        if i >= chunks.len() { break; }

        let age = now.duration_since(msg.timestamp).as_secs_f64();
        let position = msg_count - 1 - i;
        let brightness = fade(position, age);

        let p = Paragraph::new(Line::from(vec![Span::styled(
            &msg.text,
            Style::new().fg(brightness),
        )]))
        .wrap(Wrap { trim: false });

        frame.render_widget(p, chunks[i]);
    }

    let _ = total; // suppress warning
}

/// Compute text color with fading based on position in ring buffer and age.
/// Newest = full white, oldest = dim gray.
fn fade(position: usize, age_secs: f64) -> Color {
    // Position fade: newest (position=0) is bright, oldest is dim
    let pos_factor = match position {
        0 => 1.0,
        1 => 0.75,
        2 => 0.55,
        3 => 0.40,
        _ => 0.30,
    };

    // Time fade: messages older than 30s get extra dim
    let time_factor = if age_secs > 30.0 {
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
