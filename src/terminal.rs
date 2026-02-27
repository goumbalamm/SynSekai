// Crossterm terminal setup/teardown.
// Excluded from coverage (requires a real TTY) — see tarpaulin.toml.
use std::sync::Arc;

use anyhow::Result;
use crossterm::event::EventStream;
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{engine::TorrentEngine, tui::event_loop};

pub async fn run(engine: TorrentEngine) -> Result<()> {
    // Restore terminal on panic so cargo-watch (and the user) get a clean shell back.
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
            crossterm::event::DisableBracketedPaste,
        );
        orig_hook(info);
    }));

    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
        crossterm::event::EnableBracketedPaste,
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = event_loop(&mut terminal, Arc::new(engine), EventStream::new()).await;

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste,
    )?;
    terminal.show_cursor()?;

    result
}
