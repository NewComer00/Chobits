mod tcp;
mod ui;

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Terminal;
use std::io;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> io::Result<()> {
    let config = chobits::Config::load().ok();

    let port: u16 = config
        .as_ref()
        .map(|c| c.bar.port)
        .or_else(|| {
            std::env::var("CHOBITS_BAR_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
        })
        .unwrap_or(7879);
    let history_length = config
        .as_ref()
        .map(|c| c.bar.history_length)
        .unwrap_or_else(|| chobits::Config::default_config().bar.history_length);

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Spawn TCP listener for incoming text from chobits daemon
    tokio::spawn(async move {
        let addr = format!("127.0.0.1:{}", port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => listener,
            Err(e) => {
                eprintln!("[chobits-bar] Failed to bind {}: {}", addr, e);
                return;
            }
        };

        loop {
            if let Ok((mut stream, _peer)) = listener.accept().await {
                if let Ok(Some(text)) = tcp::read_message(&mut stream).await {
                    let _ = tx.send(text);
                }
            }
        }
    });

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app_state = ui::AppState::new(history_length);

    // Main loop
    loop {
        let area = terminal.size().unwrap();
        let inner_height = area.height.saturating_sub(2);
        let inner_width = area.width.saturating_sub(2) as usize;

        // Check for new messages from TCP
        while let Ok(text) = rx.try_recv() {
            app_state.push(text, inner_height, inner_width);
        }

        // Draw
        terminal.draw(|frame| ui::draw(frame, &app_state)).unwrap();

        if event::poll(std::time::Duration::from_millis(50)).unwrap() {
            match event::read().unwrap() {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    _ => {}
                },
                Event::Mouse(mouse) => {
                    let scroll_lines = 3;
                    let max_scroll = ui::max_scroll_offset(&app_state, inner_height, inner_width);
                    match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            app_state.scroll_up(scroll_lines, max_scroll);
                        }
                        MouseEventKind::ScrollDown => {
                            app_state.scroll_down(scroll_lines);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;

    Ok(())
}
