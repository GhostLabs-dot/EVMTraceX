use crate::app::{App, Panel};
use crate::model::{CallKind, TraceNode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let root = f.area();
    let header = Rect { x: root.x, y: root.y, width: root.width, height: 3.min(root.height) };
    let footer = Rect { x: root.x, y: root.y.saturating_add(root.height.saturating_sub(2)), width: root.width, height: root.height.min(2) };
    let body = Rect { x: root.x, y: root.y.saturating_add(3), width: root.width, height: root.height.saturating_sub(5) };

    draw_header(f, app, header);
    let chunks = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(61), Constraint::Percentage(39)]).split(body);
    draw_trace(f, app, chunks[0]);
    let right = Layout::default().direction(Direction::Vertical).constraints([Constraint::Percentage(56), Constraint::Percentage(44)]).split(chunks[1]);
    draw_details(f, app, right[0]);
    draw_gas(f, app, right[1]);
    draw_footer(f, footer);

    if app.show_help { draw_help(f, root); }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let s = app.stats();
    let title = format!(" evmtrace-tui  |  {} frames · {} reverts · {} storage writes · {} gas ", s.frames, s.reverts, s.storage_writes, fmt_num(s.gas_used));
    let tabs = Tabs::new([" TRACE ", " STORAGE ", " DETAILS "])
        .select(match app.active_panel { Panel::Trace => 0, Panel::Storage => 1, Panel::Details => 2 })
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

fn draw_trace(f: &mut Frame, app: &mut App, area: Rect) {
    let mut items = Vec::new();
    for id in &app.rows {
        if let Some(node) = find_node(&app.root, *id) {
            let indent = "  ".repeat(node.depth);
            let branch = if node.depth == 0 { "" } else { "└─ " };
            let status = node.status();
            let selector = node.selector().map(|s| format!("0x{}", s)).unwrap_or_else(|| "".into());
            let target = short_addr(&node.to);
            let label = format!("{}{}{:11} {:18} {:10} {:>10} gas {}", indent, branch, node.kind.as_str(), target, selector, status, fmt_num(node.gas_used));
            items.push(ListItem::new(Line::from(vec![
                Span::raw(label),
            ])));
        }
    }
    let filter_label = if app.input_mode { format!(" filter: {}█ ", app.filter.text) } else if app.filter.text.is_empty() { " Execution Trace ".into() } else { format!(" Execution Trace · filter: {} ", app.filter.text) };
    let block = Block::default().title(filter_label).borders(Borders::ALL);
    let list = List::new(items).block(block).highlight_symbol("▶ ").highlight_style(Style::default().add_modifier(Modifier::BOLD));
    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_details(f: &mut Frame, app: &App, area: Rect) {
    let Some(node) = app.selected_node() else {
        f.render_widget(Paragraph::new("No matching frames").block(Block::default().title(" Details ").borders(Borders::ALL)), area);
        return;
    };
    let selector = node.selector().map(|s| format!("0x{}", s)).unwrap_or_else(|| "—".into());
    let decoded = app.selector_name(node).unwrap_or_else(|| "unknown selector".into());
    let mut lines = vec![
        Line::from(format!("kind        {}", node.kind.as_str())),
        Line::from(format!("status      {}", node.status())),
        Line::from(format!("from        {}", full_or_dash(&node.from))),
        Line::from(format!("to          {}", full_or_dash(&node.to))),
        Line::from(format!("selector    {}", selector)),
        Line::from(format!("decoded     {}", decoded)),
        Line::from(format!("value       {}", full_or_dash(&node.value))),
        Line::from(format!("gas         {} / {}", fmt_num(node.gas_used), fmt_num(node.gas))),
        Line::from(format!("input       {} bytes", node.input_bytes_len())),
        Line::from(format!("children    {}", node.calls.len())),
    ];
    if let Some(e) = &node.error { lines.push(Line::from(format!("error       {}", e))); }
    if let Some(r) = &node.revert_reason { lines.push(Line::from(format!("revert      {}", r))); }
    let title = if !node.storage_diff.is_empty() { format!(" Details · {} storage changes ", node.storage_diff.len()) } else { " Details ".into() };
    f.render_widget(Paragraph::new(lines).block(Block::default().title(title).borders(Borders::ALL)).wrap(Wrap { trim: false }), area);
}

fn draw_gas(f: &mut Frame, app: &App, area: Rect) {
    let Some(node) = app.selected_node() else {
        f.render_widget(Block::default().title(" Gas / Storage ").borders(Borders::ALL), area);
        return;
    };
    let ratio = if node.gas == 0 { 0.0 } else { (node.gas_used as f64 / node.gas as f64).min(1.0) };
    let chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(3), Constraint::Min(4)]).split(area);
    let gauge = Gauge::default().block(Block::default().title(" Frame Gas ").borders(Borders::ALL)).ratio(ratio);
    f.render_widget(gauge, chunks[0]);
    let mut lines = vec![Line::from("Storage changes")];
    if node.storage_diff.is_empty() {
        lines.push(Line::from("  none"));
    } else {
        for change in node.storage_diff.iter().take(12) {
            lines.push(Line::from(format!("  {}", short_slot(&change.slot))));
            lines.push(Line::from(format!("    {} → {}", short_hex(&change.before), short_hex(&change.after))));
        }
        if node.storage_diff.len() > 12 { lines.push(Line::from(format!("  … {} more", node.storage_diff.len() - 12))); }
    }
    f.render_widget(Paragraph::new(lines).block(Block::default().title(" Storage Diff ").borders(Borders::ALL)).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let text = " ↑/k ↓/j move   ←/h →/l panel   / filter   e errors   w writes   r reset   ? help   q quit ";
    f.render_widget(Paragraph::new(text).block(Block::default().borders(Borders::TOP)), area);
}

fn draw_help(f: &mut Frame, root: Rect) {
    let area = Rect { x: root.x + root.width / 8, y: root.y + root.height / 7, width: root.width * 3 / 4, height: root.height * 5 / 7 };
    let text = vec![
        Line::from("Navigation"),
        Line::from("  ↑/k, ↓/j   move through frames"),
        Line::from("  ←/h, →/l   switch panel"),
        Line::from("  Enter       inspect selected frame"),
        Line::from(""),
        Line::from("Filters"),
        Line::from("  /           type an address / selector filter"),
        Line::from("  e           reverts only"),
        Line::from("  w           frames with storage changes"),
        Line::from("  r           clear filters"),
        Line::from(""),
        Line::from("Other"),
        Line::from("  ?           this help"),
        Line::from("  q / Esc     quit / close help"),
        Line::from(""),
        Line::from("Input: Geth-style callTracer JSON, or the normalized trace schema shipped in examples/.")
    ];
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(text).block(Block::default().title(" Help ").borders(Borders::ALL)).wrap(Wrap { trim: false }), area);
}

fn find_node<'a>(node: &'a TraceNode, id: usize) -> Option<&'a TraceNode> {
    if node.id == id { return Some(node); }
    for child in &node.calls { if let Some(found) = find_node(child, id) { return Some(found); } }
    None
}

fn short_addr(s: &str) -> String {
    if s.len() <= 14 { return s.to_string(); }
    format!("{}…{}", &s[..8], &s[s.len()-6..])
}
fn full_or_dash(s: &str) -> &str { if s.is_empty() { "—" } else { s } }
fn short_hex(s: &str) -> String { if s.len() <= 20 { s.to_string() } else { format!("{}…{}", &s[..10], &s[s.len()-8..]) } }
fn short_slot(s: &str) -> String { short_hex(s) }
fn fmt_num(n: u64) -> String { format_with_commas(n) }
fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len()/3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    out
}

#[allow(dead_code)]
fn _kind_color(_kind: &CallKind) {}
