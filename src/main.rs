mod app;
mod model;
mod parser;
mod selector;
mod ui;

use anyhow::Result;
use app::{App, Panel};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use parser::{load_selectors, load_trace};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, path::PathBuf};

#[derive(Parser, Debug)]
#[command(name = "evmtrace-tui", version, about = "Terminal-first EVM transaction trace explorer")]
struct Args {
    /// Path to a Geth callTracer-style JSON trace.
    input: PathBuf,
    /// Optional Solidity/Foundry ABI JSON for selector decoding.
    #[arg(short, long)]
    abi: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = load_trace(&args.input)?;
    let selectors = load_selectors(args.abi.as_deref())?;
    run(root, selectors)
}

fn run(root: model::TraceNode, selectors: std::collections::BTreeMap<String, String>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(root, selectors);
    let result = event_loop(&mut terminal, &mut app);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn event_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        if let Event::Key(key) = event::read()? {
            if handle_key(app, key)? { break; }
        }
    }
    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    if app.show_help {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) { app.show_help = false; }
        return Ok(false);
    }

    if app.input_mode {
        match key.code {
            KeyCode::Esc => {
                app.input_mode = false;
            }
            KeyCode::Enter => {
                app.input_mode = false;
                app.rebuild_rows();
            }
            KeyCode::Backspace => {
                app.filter.text.pop();
                app.rebuild_rows();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.filter.text.clear();
                app.input_mode = false;
                app.rebuild_rows();
            }
            KeyCode::Char(ch) => {
                app.filter.text.push(ch);
                app.rebuild_rows();
            }
            _ => {}
        }
        return Ok(false);
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
        KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
        KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
        KeyCode::Left | KeyCode::Char('h') => app.active_panel = Panel::Trace,
        KeyCode::Right | KeyCode::Char('l') => app.active_panel = Panel::Details,
        KeyCode::Char('?') => app.show_help = true,
        KeyCode::Char('e') => { app.filter.errors_only = !app.filter.errors_only; app.rebuild_rows(); },
        KeyCode::Char('w') => { app.filter.writes_only = !app.filter.writes_only; app.rebuild_rows(); },
        KeyCode::Char('r') => { app.filter = Default::default(); app.rebuild_rows(); },
        KeyCode::Char('/') => { app.input_mode = true; app.status_line = "Filter mode".into(); },
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => { app.filter.text.clear(); app.rebuild_rows(); },
        _ => {}
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_trace;
    use crate::selector::selector4;

    #[test]
    fn parses_nested_trace_and_storage() {
        let raw = include_str!("../examples/sample_trace.json");
        let root = parse_trace(raw).unwrap();
        assert_eq!(root.calls.len(), 3);
        assert_eq!(root.calls[1].status(), "REVERT");
        assert_eq!(root.calls[0].storage_diff.len(), 1);
        assert_eq!(root.calls[2].calls[0].depth, 2);
    }

    #[test]
    fn selector_matches_known_erc20() {
        assert_eq!(selector4("transfer(address,uint256)"), "a9059cbb");
    }
}
