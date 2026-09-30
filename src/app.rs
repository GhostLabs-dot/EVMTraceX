use crate::{model::{CallKind, Stats, TraceNode}, selector};
use ratatui::widgets::ListState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel { Trace, Storage, Details }

pub struct App {
    pub root: TraceNode,
    pub rows: Vec<usize>,
    pub selected: usize,
    pub list_state: ListState,
    pub active_panel: Panel,
    pub filter: Filter,
    pub show_help: bool,
    pub selectors: std::collections::BTreeMap<String, String>,
    pub status_line: String,
    pub input_mode: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub text: String,
    pub errors_only: bool,
    pub writes_only: bool,
    pub kind: Option<CallKind>,
}

impl App {
    pub fn new(root: TraceNode, selectors: std::collections::BTreeMap<String, String>) -> Self {
        let mut app = Self {
            root,
            rows: Vec::new(),
            selected: 0,
            list_state: ListState::default(),
            active_panel: Panel::Trace,
            filter: Filter::default(),
            show_help: false,
            selectors,
            status_line: "Ready".into(),
            input_mode: false,
        };
        app.rebuild_rows();
        app
    }

    pub fn rebuild_rows(&mut self) {
        self.rows.clear();
        let filter = self.filter.clone();
        collect_ids(&self.root, &filter, &mut self.rows);
        if self.rows.is_empty() {
            self.selected = 0;
            self.list_state.select(None);
        } else {
            self.selected = self.selected.min(self.rows.len() - 1);
            self.list_state.select(Some(self.selected));
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.rows.is_empty() { return; }
        let len = self.rows.len() as i32;
        self.selected = ((self.selected as i32 + delta).rem_euclid(len)) as usize;
        self.list_state.select(Some(self.selected));
    }

    pub fn selected_node(&self) -> Option<&TraceNode> {
        let id = *self.rows.get(self.selected)?;
        find_node(&self.root, id)
    }

    pub fn selected_node_mut(&mut self) -> Option<&mut TraceNode> {
        let id = *self.rows.get(self.selected)?;
        find_node_mut(&mut self.root, id)
    }

    pub fn selector_name(&self, node: &TraceNode) -> Option<String> {
        let selector = node.selector()?.to_ascii_lowercase();
        self.selectors.get(&selector).cloned().or_else(|| selector::builtin_selector(&selector).map(ToOwned::to_owned))
    }

    pub fn stats(&self) -> Stats { Stats::from_root(&self.root) }
}

fn collect_ids(node: &TraceNode, filter: &Filter, out: &mut Vec<usize>) {
    let mut include = true;
    if filter.errors_only && node.status() != "REVERT" { include = false; }
    if filter.writes_only && node.storage_diff.is_empty() { include = false; }
    if let Some(kind) = &filter.kind {
        if &node.kind != kind { include = false; }
    }
    if !filter.text.is_empty() {
        let q = filter.text.to_ascii_lowercase();
        include &= node.to.to_ascii_lowercase().contains(&q)
            || node.from.to_ascii_lowercase().contains(&q)
            || node.selector().unwrap_or("").to_ascii_lowercase().contains(&q);
    }
    if include { out.push(node.id); }
    for child in &node.calls { collect_ids(child, filter, out); }
}

fn find_node<'a>(node: &'a TraceNode, id: usize) -> Option<&'a TraceNode> {
    if node.id == id { return Some(node); }
    for child in &node.calls { if let Some(found) = find_node(child, id) { return Some(found); } }
    None
}

fn find_node_mut<'a>(node: &'a mut TraceNode, id: usize) -> Option<&'a mut TraceNode> {
    if node.id == id { return Some(node); }
    for child in &mut node.calls { if let Some(found) = find_node_mut(child, id) { return Some(found); } }
    None
}
