mod ui;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Terminal;
use std::io;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> io::Result<()> {
    // Read port from config, env, or default
    let port: u16 = chobits::Config::load()
        .map(|c| c.ports.bar)
        .ok()
        .or_else(|| {
            std::env::var("CHOBITS_BAR_PORT").ok().and_then(|v| v.parse().ok())
        })
        .unwrap_or(7879);

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Spawn TCP listener for incoming text from chobits daemon
    tokio::spawn(async move {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr).await.unwrap();

        loop {
            if let Ok((mut stream, _peer)) = listener.accept().await {
                let mut buf = String::new();
                if stream.read_to_string(&mut buf).await.is_ok() {
                    let text = buf.trim().to_string();
                    if !text.is_empty() {
                        let _ = tx.send(text);
                    }
                }
            }
        }
    });

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app_state = ui::AppState::new();

    // Main loop
    loop {
        // Check for new messages from TCP
        while let Ok(text) = rx.try_recv() {
            app_state.push(text);
        }

        // Draw
        terminal
            .draw(|frame| ui::draw(frame, &app_state))
            .unwrap();

        // Check for quit event (q or Ctrl+C)
        if event::poll(std::time::Duration::from_millis(50)).unwrap() {
            if let Ok(Event::Key(key)) = event::read() {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        break
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}
