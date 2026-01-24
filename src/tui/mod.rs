mod app;
pub mod messages;
mod ui;
pub mod views;
pub mod widgets;

pub use messages::{agent_channel, AgentMessage, AgentReceiver, AgentSender};

pub use app::{App, ActiveTab};
pub use ui::draw;

use std::io;
use std::panic;
use std::time::Duration;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    event::{self, Event, KeyEventKind},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use tokio::sync::mpsc::Receiver;

pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

pub fn setup_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

pub fn restore_terminal(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

/// Run the TUI event loop with panic safety.
pub async fn run(
    terminal: &mut Tui,
    app: &mut App,
    agent_rx: &mut Receiver<AgentMessage>,
) -> anyhow::Result<()> {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let result = run_loop(terminal, app, agent_rx).await;

    let _ = panic::take_hook();

    result
}

async fn run_loop(
    terminal: &mut Tui,
    app: &mut App,
    agent_rx: &mut Receiver<AgentMessage>,
) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(Duration::from_millis(50));

    loop {
        if !app.running {
            break;
        }

        terminal.draw(|frame| crate::tui::ui::draw(frame, app))?;

        tokio::select! {
            _ = interval.tick() => {
                app.spinner.tick();

                while event::poll(Duration::ZERO)? {
                    match event::read()? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            app.handle_key(key);
                        }
                        Event::Resize(_, _) => {
                            // Redraw handled next iteration
                        }
                        _ => {}
                    }
                }
            }

            Some(msg) = agent_rx.recv() => {
                app.handle_agent_message(msg);
            }
        }
    }
    Ok(())
}
