mod app;
mod ui;

pub use app::{App, ActiveTab};

use std::io;
use std::panic;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    event::{self, Event, KeyEventKind},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

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
/// Terminal is always restored even on panic or error.
pub fn run(terminal: &mut Tui, app: &mut App) -> anyhow::Result<()> {
    // Set panic hook to restore terminal
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let result = run_loop(terminal, app);

    // Restore original panic hook
    let _ = panic::take_hook();

    result
}

fn run_loop(terminal: &mut Tui, app: &mut App) -> anyhow::Result<()> {
    while app.running {
        terminal.draw(|frame| crate::tui::ui::draw(frame, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key);
                }
                Event::Resize(_, _) => {
                    // Terminal will redraw on next iteration
                }
                _ => {}
            }
        }
    }
    Ok(())
}
