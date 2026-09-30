use crate::app::{App, Panel};
use crate::model::{CallKind, TraceNode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame,
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let root = f.area();

    let header = Rect {
        x: root.x,
        y: root.y,
        width: root.width,
        height: 3.min(root.height),
    };

    let footer = Rect {
        x: root.x,
        y: root.y.saturating_add(root.height.saturating_sub(2)),
        width: root.width,
        height: root.height.min(2),
    };

    let body = Rect {
        x: root.x,
        y: root.y.saturating_add(3),
        width: root.width,
        height: root.height.saturating_sub(5),
    };

    draw_header(f, app, header);

    match app.active_panel {
        Panel::Trace => {
            draw_trace(f, app, body);
        }
        Panel::Storage => {
            draw_storage(f, app, body);
        }
        Panel::Details => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(body);

            draw_details(f, app, chunks[0]);

            let right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(4), Constraint::Min(4)])
                .split(chunks[1]);

            draw_gas(f, app, right[0]);
            draw_storage(f, app, right[1]);
        }
    }

    draw_footer(f, app, footer);

    if app.show_help {
        draw_help(f, root);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let s = app.stats();

    let title = format!(
        " evmtrace-tui  │  {} frames  │  {} reverts  │  {} storage writes  │  tx gas {} ",
        s.frames,
        s.reverts,
        s.storage_writes,
        fmt_num(s.gas_used)
    );

    let tabs = Tabs::new([
        Span::styled(" TRACE ", Style::default().fg(Color::Cyan)),
        Span::styled(" STORAGE ", Style::default().fg(Color::Magenta)),
        Span::styled(" DETAILS ", Style::default().fg(Color::Yellow)),
    ])
    .select(match app.active_panel {
        Panel::Trace => 0,
        Panel::Storage => 1,
        Panel::Details => 2,
    })
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    )
    .highlight_style(
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    )
    .divider("│");

    f.render_widget(tabs, area);
}

fn draw_trace(f: &mut Frame, app: &mut App, area: Rect) {
    let mut items = Vec::new();

    for id in &app.rows {
        if let Some(node) = find_node(&app.root, *id) {
            let indent = "  ".repeat(node.depth);
            let branch = if node.depth == 0 { "" } else { "└─ " };
            let status = node.status();
            let selector = node.selector().map(|s| format!("0x{}", s)).unwrap_or_default();
            let target = short_addr(&node.to);

            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("{}{}", indent, branch), Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:11}", node.kind.as_str()), kind_style(&node.kind)),
                Span::styled(format!(" {:18}", target), Style::default().fg(Color::White)),
                Span::styled(format!(" {:10}", selector), Style::default().fg(Color::Cyan)),
                Span::styled(format!(" {:>10}", status), status_style(status)),
                Span::styled(
                    format!(" gas {}", fmt_num(node.gas_used)),
                    Style::default().fg(Color::DarkGray),
                ),
            ])));
        }
    }

    let filter_label = if app.input_mode {
        format!(" filter: {}█ ", app.filter.text)
    } else if app.filter.text.is_empty() {
        " Execution Trace ".into()
    } else {
        format!(" Execution Trace · filter: {} ", app.filter.text)
    };

    let block = Block::default()
        .title(filter_label)
        .borders(Borders::ALL)
        .border_style(panel_border(app.active_panel == Panel::Trace, Color::Cyan));

    let list = List::new(items).block(block).highlight_symbol("▶ ").highlight_style(
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    );

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_storage(f: &mut Frame, app: &mut App, area: Rect) {
    let mut items = Vec::new();
    let mut frame_ids = Vec::new();
    collect_storage_rows(&app.root, &mut items, &mut frame_ids);

    if items.is_empty() {
        f.render_widget(
            Paragraph::new("No storage changes in this trace")
                .block(
                    Block::default()
                        .title(" Storage ")
                        .borders(Borders::ALL)
                        .border_style(panel_border(app.active_panel == Panel::Storage, Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }

    let block = Block::default()
        .title(format!(" Storage · {} changes ", frame_ids.len()))
        .borders(Borders::ALL)
        .border_style(panel_border(app.active_panel == Panel::Storage, Color::Magenta));

    let list = List::new(items).block(block).highlight_symbol("▶ ").highlight_style(
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    );

    let selected_id = app.selected_node().map(|node| node.id);
    let mut storage_state = ListState::default();

    if let Some(id) = selected_id {
        if let Some(index) = frame_ids.iter().position(|frame_id| *frame_id == id) {
            storage_state.select(Some(index));
        }
    }

    f.render_stateful_widget(list, area, &mut storage_state);
}

fn collect_storage_rows(node: &TraceNode, out: &mut Vec<ListItem<'static>>, frame_ids: &mut Vec<usize>) {
    for change in &node.storage_diff {
        out.push(ListItem::new(Line::from(vec![
            Span::styled(format!("#{} ", node.id), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:12} ", node.kind.as_str()), kind_style(&node.kind)),
            Span::styled(format!("{} ", short_addr(&node.to)), Style::default().fg(Color::White)),
            Span::styled(
                format!("slot {}", short_slot(&change.slot)),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!(" {} → {}", short_hex(&change.before), short_hex(&change.after)),
                Style::default().fg(Color::White),
            ),
        ])));

        frame_ids.push(node.id);
    }

    for child in &node.calls {
        collect_storage_rows(child, out, frame_ids);
    }
}

fn draw_details(f: &mut Frame, app: &App, area: Rect) {
    let Some(node) = app.selected_node() else {
        f.render_widget(
            Paragraph::new("No matching frames").block(
                Block::default()
                    .title(" Details ")
                    .borders(Borders::ALL)
                    .border_style(panel_border(app.active_panel == Panel::Details, Color::Yellow)),
            ),
            area,
        );
        return;
    };

    let selector = node
        .selector()
        .map(|s| format!("0x{}", s))
        .unwrap_or_else(|| "—".into());

    let decoded = app.selector_name(node).unwrap_or_else(|| "unknown selector".into());

    let mut lines = vec![
        Line::from(vec![
            Span::styled("kind        ", Style::default().fg(Color::DarkGray)),
            Span::styled(node.kind.as_str(), kind_style(&node.kind)),
        ]),
        Line::from(vec![
            Span::styled("status      ", Style::default().fg(Color::DarkGray)),
            Span::styled(node.status(), status_style(node.status())),
        ]),
        Line::from(vec![
            Span::styled("from        ", Style::default().fg(Color::DarkGray)),
            Span::styled(full_or_dash(&node.from), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("to          ", Style::default().fg(Color::DarkGray)),
            Span::styled(full_or_dash(&node.to), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("selector    ", Style::default().fg(Color::DarkGray)),
            Span::styled(selector, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("decoded     ", Style::default().fg(Color::DarkGray)),
            Span::styled(decoded, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("value       ", Style::default().fg(Color::DarkGray)),
            Span::styled(full_or_dash(&node.value), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("gas         ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} / {}", fmt_num(node.gas_used), fmt_num(node.gas)),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(vec![
            Span::styled("input       ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} bytes", node.input_bytes_len()),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("children    ", Style::default().fg(Color::DarkGray)),
            Span::styled(node.calls.len().to_string(), Style::default().fg(Color::White)),
        ]),
    ];

    if let Some(e) = &node.error {
        lines.push(Line::from(vec![
            Span::styled("error       ", Style::default().fg(Color::DarkGray)),
            Span::styled(e, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]));
    }

    if let Some(r) = &node.revert_reason {
        lines.push(Line::from(vec![
            Span::styled("revert      ", Style::default().fg(Color::DarkGray)),
            Span::styled(r, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]));
    }

    let title = if !node.storage_diff.is_empty() {
        format!(" Details · {} storage changes ", node.storage_diff.len())
    } else {
        " Details ".into()
    };

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_gas(f: &mut Frame, app: &App, area: Rect) {
    let Some(node) = app.selected_node() else {
        f.render_widget(
            Block::default()
                .title(" Gas / Storage ")
                .borders(Borders::ALL)
                .border_style(panel_border(app.active_panel == Panel::Storage, Color::Magenta)),
            area,
        );
        return;
    };

    let ratio = if node.gas == 0 {
        0.0
    } else {
        (node.gas_used as f64 / node.gas as f64).min(1.0)
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area);

    let gas_percent = format!("{:.1}%", ratio * 100.0);

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(" Frame Gas ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .gauge_style(Style::default().fg(Color::Cyan))
        .label(gas_percent)
        .ratio(ratio);

    f.render_widget(gauge, chunks[0]);

    let mut lines = vec![Line::from(Span::styled(
        "Storage changes",
        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
    ))];

    if node.storage_diff.is_empty() {
        lines.push(Line::from(Span::styled("  none", Style::default().fg(Color::DarkGray))));
    } else {
        for change in node.storage_diff.iter().take(12) {
            lines.push(Line::from(Span::styled(
                format!("  {}", short_slot(&change.slot)),
                Style::default().fg(Color::Yellow),
            )));

            lines.push(Line::from(vec![
                Span::styled(
                    format!("    {}", short_hex(&change.before)),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(" → ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(short_hex(&change.after), Style::default().fg(Color::Green)),
            ]));
        }

        if node.storage_diff.len() > 12 {
            lines.push(Line::from(Span::styled(
                format!("  … {} more", node.storage_diff.len() - 12),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" Storage Diff ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .wrap(Wrap { trim: false }),
        chunks[1],
    );
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let panel = match app.active_panel {
        Panel::Trace => "TRACE",
        Panel::Storage => "STORAGE",
        Panel::Details => "DETAILS",
    };

    let status = if app.rows.is_empty() {
        format!("No matching frames · {} · {}", panel, app.status_line)
    } else {
        format!(
            "Frame {}/{} · {} · {}",
            app.selected + 1,
            app.rows.len(),
            panel,
            app.status_line
        )
    };

    let shortcuts = Line::from(vec![
        Span::styled(" ↑/k ↓/j ", Style::default().fg(Color::Yellow)),
        Span::styled("move   ", Style::default().fg(Color::DarkGray)),
        Span::styled("←/h →/l ", Style::default().fg(Color::Yellow)),
        Span::styled("panel   ", Style::default().fg(Color::DarkGray)),
        Span::styled("/ ", Style::default().fg(Color::Cyan)),
        Span::styled("filter   ", Style::default().fg(Color::DarkGray)),
        Span::styled("e ", Style::default().fg(Color::Red)),
        Span::styled("errors   ", Style::default().fg(Color::DarkGray)),
        Span::styled("w ", Style::default().fg(Color::Green)),
        Span::styled("writes   ", Style::default().fg(Color::DarkGray)),
        Span::styled("r ", Style::default().fg(Color::Yellow)),
        Span::styled("reset   ", Style::default().fg(Color::DarkGray)),
        Span::styled("? ", Style::default().fg(Color::Magenta)),
        Span::styled("help   ", Style::default().fg(Color::DarkGray)),
        Span::styled("q ", Style::default().fg(Color::Red)),
        Span::styled("quit", Style::default().fg(Color::DarkGray)),
    ]);

    f.render_widget(
        Paragraph::new(vec![
            shortcuts,
            Line::from(Span::styled(status, Style::default().fg(Color::DarkGray))),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        area,
    );
}

fn draw_help(f: &mut Frame, root: Rect) {
    let area = Rect {
        x: root.x + root.width / 8,
        y: root.y + root.height / 7,
        width: root.width * 3 / 4,
        height: root.height * 5 / 7,
    };

    let text = vec![
        Line::from(Span::styled(
            "Navigation",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from("  ↑/k, ↓/j           move through frames"),
        Line::from("  ←/h, →/l           switch panel"),
        Line::from("  Tab / Shift+Tab    next / previous panel"),
        Line::from("  Home / End         first / last frame"),
        Line::from("  PageUp / PageDown  move by 10 frames"),
        Line::from("  Enter              open Details"),
        Line::from(""),
        Line::from(Span::styled(
            "Filters",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )),
        Line::from("  /           type an address / selector filter"),
        Line::from("  e           reverts only"),
        Line::from("  w           frames with storage changes"),
        Line::from("  r           clear filters"),
        Line::from(""),
        Line::from(Span::styled(
            "Other",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from("  ?           this help"),
        Line::from("  q / Esc     quit / close help"),
        Line::from(""),
        Line::from("Input: Geth-style callTracer JSON, or the normalized trace schema shipped in examples/."),
    ];

    f.render_widget(Clear, area);

    f.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .title(" Help ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn find_node(node: &TraceNode, id: usize) -> Option<&TraceNode> {
    if node.id == id {
        return Some(node);
    }

    for child in &node.calls {
        if let Some(found) = find_node(child, id) {
            return Some(found);
        }
    }

    None
}

fn panel_border(active: bool, accent: Color) -> Style {
    if active {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn kind_style(kind: &CallKind) -> Style {
    match kind {
        CallKind::Call => Style::default().fg(Color::Cyan),
        CallKind::StaticCall => Style::default().fg(Color::Magenta),
        CallKind::DelegateCall => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        CallKind::CallCode => Style::default().fg(Color::LightYellow),
        CallKind::Create => Style::default().fg(Color::Green),
        CallKind::Create2 => Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD),
        CallKind::SelfDestruct => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        CallKind::Unknown => Style::default().fg(Color::Gray),
    }
}

fn status_style(status: &str) -> Style {
    match status {
        "REVERT" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        "SUCCESS" => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        _ => Style::default().fg(Color::Gray),
    }
}

fn short_addr(s: &str) -> String {
    if s.len() <= 14 {
        return s.to_string();
    }

    format!("{}…{}", &s[..8], &s[s.len() - 6..])
}

fn full_or_dash(s: &str) -> &str {
    if s.is_empty() {
        "—"
    } else {
        s
    }
}

fn short_hex(s: &str) -> String {
    if s.len() <= 20 {
        s.to_string()
    } else {
        format!("{}…{}", &s[..10], &s[s.len() - 8..])
    }
}

fn short_slot(s: &str) -> String {
    short_hex(s)
}

fn fmt_num(n: u64) -> String {
    format_with_commas(n)
}

fn format_with_commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);

    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }

        out.push(ch);
    }

    out
}
