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
use model::TraceDocument;
use parser::{load_foundry_trace, load_selectors, load_trace, load_trace_rpc};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, path::PathBuf, time::Duration};

#[derive(Parser, Debug)]
#[command(
    name = "evmtrace-tui",
    version,
    about = "EVM Trace TUI — terminal-first EVM transaction trace explorer"
)]
struct Args {
    /// Path to a Geth callTracer-style JSON trace.
    input: Option<PathBuf>,
    /// Path to Foundry `forge test -vvvv` output.
    #[arg(long, conflicts_with_all = ["input", "tx", "rpc"], value_name = "FILE")]
    foundry: Option<PathBuf>,
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

    let root = if let Some(path) = &args.foundry {
        load_foundry_trace(path)?
    } else {
        match (&args.input, &args.tx, &args.rpc) {
            (Some(path), None, None) => load_trace(path)?,
            (None, Some(tx_hash), Some(rpc_url)) => load_trace_rpc(rpc_url, tx_hash)?,
            _ => bail!("provide either a trace file, a Foundry trace, or both --tx and --rpc"),
        }
    };

    let selectors = load_selectors(args.abi.as_deref())?;
    run(root, selectors)
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen);
    }
}

fn run(document: TraceDocument, selectors: std::collections::BTreeMap<String, String>) -> Result<()> {
    enable_raw_mode()?;
    let _terminal_guard = TerminalGuard;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new(document, selectors);
    let result = event_loop(&mut terminal, &mut app);
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
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Char('Q') => {
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
                if let Some(previous) = app.filter_input_backup.take() {
                    app.filter.text = previous;
                    app.rebuild_rows();
                }
                app.input_mode = false;
                app.status_line = format!("Filter canceled · {} frames", app.rows.len());
            }
            KeyCode::Enter => {
                app.filter_input_backup = None;
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
                app.filter_input_backup = None;
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
            app.status_line = selection_status(app);
        }
        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
            app.move_selection(-1);
            app.status_line = selection_status(app);
        }
        KeyCode::Home => {
            app.select_first();
            app.status_line = selection_status(app);
        }
        KeyCode::End => {
            app.select_last();
            app.status_line = selection_status(app);
        }
        KeyCode::PageDown => {
            app.move_selection(10);
            app.status_line = selection_status(app);
        }
        KeyCode::PageUp => {
            app.move_selection(-10);
            app.status_line = selection_status(app);
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
            app.filter_input_backup = Some(app.filter.text.clone());
            app.input_mode = true;
            app.status_line = "Filter mode".into();
        }
        _ => {}
    }

    Ok(false)
}

fn selection_status(app: &App) -> String {
    if app.rows.is_empty() {
        "No matching frames".into()
    } else {
        format!("Frame {}/{}", app.selected + 1, app.rows.len())
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Panel};
    use crate::handle_key;
    use crate::parser::{parse_foundry_trace, parse_trace};
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
    fn parses_foundry_vvvvv_trace() {
        let raw = r#"
Ran 1 tests for test/Counter.t.sol:CounterTest
Traces:
  [28783] CounterTest::test_Increment()
    ├─ [22418] Counter::increment()
    │   └─ ← [Stop]
    ├─ [424] Counter::number() [staticcall]
    │   └─ ← [Return] 1
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; finished in 1.00ms
"#;

        let document = parse_foundry_trace(raw).unwrap();

        assert_eq!(document.roots.len(), 1);
        assert_eq!(
            document.roots[0].label.as_deref(),
            Some("CounterTest::test_Increment()")
        );
        assert_eq!(document.roots[0].calls.len(), 2);
        assert_eq!(document.roots[0].calls[1].kind, crate::model::CallKind::StaticCall);
        assert_eq!(document.roots[0].calls[1].gas_used, 424);
        assert_eq!(document.roots[0].calls[1].output, "1");
    }

    #[test]
    fn attributes_foundry_results_to_the_immediately_previous_frame() {
        let raw = r#"
Traces:
  [3000] Test::run()
    ├─ [1000] Contract::first()
    │   └─ ← [Return] 123
    ├─ [1200] Contract::second()
    │   └─ ← [Revert] oracle failed
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; finished in 1.00ms
"#;

        let document = parse_foundry_trace(raw).unwrap();

        assert_eq!(document.roots.len(), 1);
        assert_eq!(document.roots[0].calls.len(), 2);
        assert_eq!(document.roots[0].calls[0].output, "123");
        assert_eq!(
            document.roots[0].calls[1].revert_reason.as_deref(),
            Some("oracle failed")
        );
        assert_eq!(document.roots[0].calls[1].status(), "REVERT");
        assert_eq!(document.roots[0].status(), "SUCCESS");
    }

    #[test]
    fn attributes_foundry_parent_result_to_the_parent_frame() {
        let raw = r#"
Traces:
  [3000] Test::run()
    ├─ [1000] Contract::first()
    │   └─ ← [Stop]
    └─ ← [Revert] root failed

Suite result: FAILED. 0 passed; 1 failed; 0 skipped; finished in 1.00ms
"#;

        let document = parse_foundry_trace(raw).unwrap();

        assert_eq!(document.roots.len(), 1);
        assert_eq!(document.roots[0].status(), "REVERT");
        assert_eq!(document.roots[0].revert_reason.as_deref(), Some("root failed"));
        assert_eq!(document.roots[0].calls[0].status(), "SUCCESS");
        assert_eq!(document.outcome, Some(crate::model::TraceOutcome::Failed));
    }

    #[test]
    fn parses_multiple_foundry_trace_roots_without_merging_gas() {
        let raw = r#"
Traces:
  [1000] CounterTest::test_First()
    └─ [700] Counter::increment()
    │   └─ ← [Stop]
    └─ ← [Stop]

Traces:
  [2000] CounterTest::test_Second()
    └─ [1500] Counter::number() [staticcall]
    │   └─ ← [Return] 42
    └─ ← [Stop]

Suite result: ok. 2 passed; 0 failed; 0 skipped; finished in 1.00ms
"#;

        let document = parse_foundry_trace(raw).unwrap();

        assert_eq!(document.roots.len(), 2);
        assert_eq!(document.roots[0].label.as_deref(), Some("CounterTest::test_First()"));
        assert_eq!(document.roots[1].label.as_deref(), Some("CounterTest::test_Second()"));
        assert_eq!(document.roots[0].gas_used, 1000);
        assert_eq!(document.roots[1].gas_used, 2000);
        assert_eq!(document.roots[1].calls[0].output, "42");
    }

    #[test]
    fn foundry_cheatcodes_are_not_classified_as_staticcalls() {
        let raw = r#"
Traces:
  [3000] Test::run()
    ├─ [1000] 0x1111111111111111111111111111111111111111::outer()
    │   ├─ [500] 0x2222222222222222222222222222222222222222::inner() [staticcall]
    │   │   └─ ← [Return] 1
    │   └─ [200] VM::load(0x01) [staticcall]
    └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; finished in 1.00ms
"#;

        let document = parse_foundry_trace(raw).unwrap();
        let outer = &document.roots[0].calls[0];
        let inner = &outer.calls[0];
        let vm = &outer.calls[1];

        assert_eq!(document.roots[0].kind, crate::model::CallKind::FoundryTest);
        assert_eq!(outer.to, "0x1111111111111111111111111111111111111111");
        assert_eq!(inner.to, "0x2222222222222222222222222222222222222222");
        assert_eq!(inner.from, "0x1111111111111111111111111111111111111111");
        assert_eq!(inner.kind, crate::model::CallKind::StaticCall);
        assert_eq!(vm.kind, crate::model::CallKind::Cheatcode);
        assert!(vm.to.is_empty());
        assert_eq!(document.outcome, Some(crate::model::TraceOutcome::Passed));
    }

    #[test]
    fn foundry_failed_suite_result_is_preserved() {
        let raw = r#"
Traces:
  [3000] Test::run()
    └─ [200] VM::assertEq(0, 1)

Suite result: FAILED. 0 passed; 1 failed; 0 skipped; finished in 1.00ms
"#;

        let document = parse_foundry_trace(raw).unwrap();

        assert_eq!(document.roots.len(), 1);
        assert_eq!(document.outcome, Some(crate::model::TraceOutcome::Failed));
    }

    #[test]
    fn foundry_storage_changes_are_parsed() {
        let raw = r#"
Traces:
  [3000] 0x1111111111111111111111111111111111111111::run()
    └─ [1000] 0x2222222222222222222222222222222222222222::set()
       - state diff:
         @ 0x01 (counter, uint256): 0 → 1
         @ 0x02: 2 -> 3
       └─ ← [Stop]

Suite result: ok. 1 passed; 0 failed; 0 skipped; finished in 1.00ms
"#;

        let document = parse_foundry_trace(raw).unwrap();
        let node = &document.roots[0].calls[0];

        assert_eq!(node.storage_diff.len(), 2);
        assert_eq!(node.storage_diff[0].slot, "0x01");
        assert_eq!(node.storage_diff[0].before, "0");
        assert_eq!(node.storage_diff[0].after, "1");
        assert_eq!(node.storage_diff[1].slot, "0x02");
        assert_eq!(node.storage_diff[1].before, "2");
        assert_eq!(node.storage_diff[1].after, "3");
    }

    #[test]
    fn malformed_selector_input_is_not_exposed() {
        let raw = r#"{
  "type": "CALL",
  "from": "0x1111111111111111111111111111111111111111",
  "to": "0x2222222222222222222222222222222222222222",
  "input": "éééééééé"
}"#;

        let root = parse_trace(raw).unwrap();

        assert_eq!(root.selector(), None);

        let raw = r#"{
  "type": "CALL",
  "from": "0x1111111111111111111111111111111111111111",
  "to": "0x2222222222222222222222222222222222222222",
  "input": "0xa9059cbbZZ"
}"#;

        let root = parse_trace(raw).unwrap();

        assert_eq!(root.selector(), None);
    }

    #[test]
    fn keyboard_navigation_changes_selection_and_panels() {
        let root = parse_trace(include_str!("../examples/sample_trace.json")).unwrap();
        let mut app = App::new(
            crate::model::TraceDocument::single(root),
            std::collections::BTreeMap::new(),
        );

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
