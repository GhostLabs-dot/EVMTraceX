mod app;
mod model;
mod parser;
mod selector;
mod ui;

use anyhow::{bail, Result};
use app::{App, Panel};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use parser::{load_selectors, load_trace, load_trace_rpc};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, path::PathBuf, time::Duration};

#[derive(Parser, Debug)]
#[command(
    name = "evmtrace-tui",
    version,
    about = "Terminal-first EVM transaction trace explorer"
)]
struct Args {
    /// Path to a Geth callTracer-style JSON trace.
    input: Option<PathBuf>,
    /// Transaction hash to trace through an RPC endpoint.
    #[arg(long, requires = "rpc", conflicts_with = "input", value_name = "TX_HASH")]
    tx: Option<String>,
    /// RPC endpoint used with --tx.
    #[arg(long, requires = "tx", value_name = "URL")]
    rpc: Option<String>,
    /// Optional Solidity/Foundry ABI JSON for selector decoding.
    #[arg(short, long)]
    abi: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let root = match (&args.input, &args.tx, &args.rpc) {
        (Some(path), None, None) => load_trace(path)?,
        (None, Some(tx_hash), Some(rpc_url)) => load_trace_rpc(rpc_url, tx_hash)?,
        _ => bail!("provide either a trace file or both --tx and --rpc"),
    };

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

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if !key.is_press() && !key.is_repeat() {
                continue;
            }

            if handle_key(app, key)? {
                break;
            }
        }
    }

    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') => {
                app.show_help = false;
                app.status_line = "Help closed".into();
            }
            _ => {}
        }
        return Ok(false);
    }

    if app.input_mode {
        match key.code {
            KeyCode::Esc => {
                app.input_mode = false;
                app.status_line = "Filter canceled".into();
            }
            KeyCode::Enter => {
                app.input_mode = false;
                app.rebuild_rows();
                app.status_line = format!("Filter applied · {} frames", app.rows.len());
            }
            KeyCode::Backspace => {
                app.filter.text.pop();
                app.rebuild_rows();
                app.status_line = format!("Filter · {} frames", app.rows.len());
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.filter.text.clear();
                app.input_mode = false;
                app.rebuild_rows();
                app.status_line = "Filter cleared".into();
            }
            KeyCode::Char(ch) => {
                app.filter.text.push(ch);
                app.rebuild_rows();
                app.status_line = format!("Filter · {} frames", app.rows.len());
            }
            _ => {}
        }

        return Ok(false);
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
            return Ok(true);
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Ok(true);
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
            app.move_selection(1);
            app.status_line = format!("Frame {}/{}", app.selected + 1, app.rows.len());
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
            app.move_selection(-1);
            app.status_line = format!("Frame {}/{}", app.selected + 1, app.rows.len());
        }
        KeyCode::Home => {
            app.select_first();
            app.status_line = format!("Frame 1/{}", app.rows.len());
        }
        KeyCode::End => {
            app.select_last();
            app.status_line = format!("Frame {}/{}", app.selected + 1, app.rows.len());
        }
        KeyCode::PageDown => {
            app.move_selection(10);
            app.status_line = format!("Frame {}/{}", app.selected + 1, app.rows.len());
        }
        KeyCode::PageUp => {
            app.move_selection(-10);
            app.status_line = format!("Frame {}/{}", app.selected + 1, app.rows.len());
        }
        KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('H') | KeyCode::BackTab => {
            app.prev_panel();
            app.status_line = "Previous panel".into();
        }
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Char('L') | KeyCode::Tab => {
            app.next_panel();
            app.status_line = "Next panel".into();
        }
        KeyCode::Enter => {
            app.active_panel = Panel::Details;
            app.status_line = "Details panel".into();
        }
        KeyCode::Char('?') => {
            app.show_help = true;
            app.status_line = "Help".into();
        }
        KeyCode::Char('e') => {
            app.filter.errors_only = !app.filter.errors_only;
            app.rebuild_rows();
            app.status_line = if app.filter.errors_only {
                format!("Reverts only · {} frames", app.rows.len())
            } else {
                "Revert filter disabled".into()
            };
        }
        KeyCode::Char('w') => {
            app.filter.writes_only = !app.filter.writes_only;
            app.rebuild_rows();
            app.status_line = if app.filter.writes_only {
                format!("Storage writes only · {} frames", app.rows.len())
            } else {
                "Storage filter disabled".into()
            };
        }
        KeyCode::Char('r') => {
            app.filter = Default::default();
            app.rebuild_rows();
            app.status_line = format!("Filters cleared · {} frames", app.rows.len());
        }
        KeyCode::Char('/') => {
            app.input_mode = true;
            app.status_line = "Filter mode".into();
        }
        _ => {}
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Panel};
    use crate::handle_key;
    use crate::parser::parse_trace;
    use crate::selector::selector4;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn parses_nested_trace_and_storage() {
        let raw = include_str!("../examples/sample_trace.json");
        let root = parse_trace(raw).unwrap();
        assert_eq!(root.calls.len(), 3);
        assert_eq!(root.calls[1].status(), "REVERT");
        assert_eq!(root.storage_diff.len(), 1);
        assert_eq!(root.calls[2].calls[0].depth, 2);
    }

    #[test]
    fn selector_matches_known_erc20() {
        assert_eq!(selector4("transfer(address,uint256)"), "a9059cbb");
    }

    #[test]
    fn keyboard_navigation_changes_selection_and_panels() {
        let root = parse_trace(include_str!("../examples/sample_trace.json")).unwrap();
        let mut app = App::new(root, std::collections::BTreeMap::new());

        assert_eq!(app.active_panel, Panel::Trace);
        assert_eq!(app.selected, 0);

        assert!(!handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)).unwrap());
        assert_eq!(app.selected, 1);

        assert!(!handle_key(&mut app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)).unwrap());
        assert_eq!(app.selected, 0);

        assert!(!handle_key(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap());
        assert_eq!(app.active_panel, Panel::Storage);

        assert!(!handle_key(&mut app, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)).unwrap());
        assert_eq!(app.active_panel, Panel::Trace);

        assert!(!handle_key(&mut app, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)).unwrap());
        assert_eq!(app.active_panel, Panel::Storage);

        assert!(!handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)).unwrap());
        assert_eq!(app.active_panel, Panel::Details);

        assert!(!handle_key(&mut app, KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE)).unwrap());
        assert_eq!(app.active_panel, Panel::Storage);

        assert!(!handle_key(&mut app, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)).unwrap());
        assert_eq!(app.selected, 0);

        assert!(!handle_key(&mut app, KeyEvent::new(KeyCode::End, KeyModifiers::NONE)).unwrap());
        assert_eq!(app.selected, app.rows.len() - 1);
    }
}
