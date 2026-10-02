use crate::{
    model::{CallKind, Stats, TraceDocument, TraceNode},
    selector,
};
use ratatui::widgets::ListState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Panel {
    Trace,
    Storage,
    Details,
}

pub struct App {
    pub document: TraceDocument,
    pub rows: Vec<usize>,
    pub selected: usize,
    pub list_state: ListState,
    pub active_panel: Panel,
    pub filter: Filter,
    pub show_help: bool,
    pub selectors: std::collections::BTreeMap<String, String>,
    pub status_line: String,
    pub input_mode: bool,
    pub filter_input_backup: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub text: String,
    pub errors_only: bool,
    pub writes_only: bool,
    pub kind: Option<CallKind>,
}

impl App {
    pub fn new(document: TraceDocument, selectors: std::collections::BTreeMap<String, String>) -> Self {
        let mut app = Self {
            document,
            rows: Vec::new(),
            selected: 0,
            list_state: ListState::default(),
            active_panel: Panel::Trace,
            filter: Filter::default(),
            show_help: false,
            selectors,
            status_line: "Ready".into(),
            input_mode: false,
            filter_input_backup: None,
        };
        app.rebuild_rows();
        app
    }

    pub fn rebuild_rows(&mut self) {
        self.rows.clear();
        let filter = self.filter.clone();

        for root in &self.document.roots {
            collect_ids(root, &filter, &mut self.rows);
        }

        if self.rows.is_empty() {
            self.selected = 0;
            self.list_state.select(None);
        } else {
            self.selected = self.selected.min(self.rows.len() - 1);
            self.list_state.select(Some(self.selected));
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        let max = (self.rows.len() - 1) as i32;
        self.selected = (self.selected as i32 + delta).clamp(0, max) as usize;
        self.list_state.select(Some(self.selected));
    }

    pub fn select_first(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = 0;
        self.list_state.select(Some(0));
    }

    pub fn select_last(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        self.selected = self.rows.len() - 1;
        self.list_state.select(Some(self.selected));
    }

    pub fn next_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Trace => Panel::Storage,
            Panel::Storage => Panel::Details,
            Panel::Details => Panel::Trace,
        };
    }

    pub fn prev_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Trace => Panel::Details,
            Panel::Storage => Panel::Trace,
            Panel::Details => Panel::Storage,
        };
    }

    pub fn selected_node(&self) -> Option<&TraceNode> {
        let id = *self.rows.get(self.selected)?;

        for root in &self.document.roots {
            if let Some(node) = find_node(root, id) {
                return Some(node);
            }
        }

        None
    }

    #[allow(dead_code)]
    pub fn selected_node_mut(&mut self) -> Option<&mut TraceNode> {
        let id = *self.rows.get(self.selected)?;

        for root in &mut self.document.roots {
            if let Some(node) = find_node_mut(root, id) {
                return Some(node);
            }
        }

        None
    }

    pub fn selector_name(&self, node: &TraceNode) -> Option<String> {
        let selector = node.selector()?.to_ascii_lowercase();
        self.selectors
            .get(&selector)
            .cloned()
            .or_else(|| selector::builtin_selector(&selector).map(ToOwned::to_owned))
    }

    pub fn stats(&self) -> Stats {
        Stats::from_document(&self.document)
    }
}

fn collect_ids(node: &TraceNode, filter: &Filter, out: &mut Vec<usize>) {
    let mut include = true;
    if filter.errors_only && node.status() != "REVERT" {
        include = false;
    }
    if filter.writes_only && node.storage_diff.is_empty() {
        include = false;
    }
    if let Some(kind) = &filter.kind {
        if &node.kind != kind {
            include = false;
        }
    }
    if !filter.text.is_empty() {
        let q = filter.text.to_ascii_lowercase();
        let selector_query = q.strip_prefix("0x").unwrap_or(&q);

        include &= node.display_name().to_ascii_lowercase().contains(&q)
            || node.to.to_ascii_lowercase().contains(&q)
            || node.from.to_ascii_lowercase().contains(&q)
            || node
                .selector()
                .unwrap_or("")
                .to_ascii_lowercase()
                .contains(selector_query);
    }
    if include {
        out.push(node.id);
    }
    for child in &node.calls {
        collect_ids(child, filter, out);
    }
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

#[allow(dead_code)]
fn find_node_mut(node: &mut TraceNode, id: usize) -> Option<&mut TraceNode> {
    if node.id == id {
        return Some(node);
    }
    for child in &mut node.calls {
        if let Some(found) = find_node_mut(child, id) {
            return Some(found);
        }
    }
    None
}
