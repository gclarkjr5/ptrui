use std::io::{self};

use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

mod api;
mod app;
mod languages;
mod textarea;
mod ui;
mod vim;

fn main() -> io::Result<()> {
    // Raw mode lets us read keys directly without line buffering.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // Switch to an alternate screen so we can draw a TUI.
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = app::run_app(&mut terminal);

    // Always restore the terminal to a clean state.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
