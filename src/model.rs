use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CallKind {
    #[serde(rename = "CALL", alias = "call")]
    Call,
    #[serde(rename = "STATICCALL", alias = "staticcall")]
    StaticCall,
    #[serde(rename = "DELEGATECALL", alias = "delegatecall")]
    DelegateCall,
    #[serde(rename = "CALLCODE", alias = "callcode")]
    CallCode,
    #[serde(rename = "CREATE", alias = "create")]
    Create,
    #[serde(rename = "CREATE2", alias = "create2")]
    Create2,
    #[serde(rename = "SELFDESTRUCT", alias = "selfdestruct")]
    SelfDestruct,
    #[serde(other)]
    Unknown,
}

impl CallKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Call => "CALL",
            Self::StaticCall => "STATICCALL",
            Self::DelegateCall => "DELEGATECALL",
            Self::CallCode => "CALLCODE",
            Self::Create => "CREATE",
            Self::Create2 => "CREATE2",
            Self::SelfDestruct => "SELFDESTRUCT",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageChange {
    pub slot: String,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceNode {
    pub id: usize,
    pub depth: usize,
    pub kind: CallKind,
    #[serde(default)]
    pub label: Option<String>,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub gas: u64,
    #[serde(rename = "gasUsed", default)]
    pub gas_used: u64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(rename = "revertReason", default)]
    pub revert_reason: Option<String>,
    #[serde(rename = "storageDiff", default)]
    pub storage_diff: Vec<StorageChange>,
    #[serde(default)]
    pub calls: Vec<TraceNode>,
}

impl TraceNode {
    pub fn status(&self) -> &'static str {
        if self.error.is_some() || self.revert_reason.is_some() {
            "REVERT"
        } else {
            "SUCCESS"
        }
    }

    pub fn selector(&self) -> Option<&str> {
        let s = self.input.strip_prefix("0x").unwrap_or(&self.input);

        if s.len() < 8 || !s.as_bytes()[..8].iter().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }

        Some(&s[..8])
    }

    pub fn input_bytes_len(&self) -> usize {
        let s = self.input.strip_prefix("0x").unwrap_or(&self.input);

        if !s.len().is_multiple_of(2) {
            return 0;
        }

        s.len() / 2
    }

    pub fn display_name(&self) -> &str {
        self.label.as_deref().filter(|s| !s.is_empty()).unwrap_or(&self.to)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceOutcome {
    Passed,
    Failed,
}

impl TraceOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Passed => "PASS",
            Self::Failed => "FAILED",
        }
    }
}

#[derive(Clone, Debug)]
pub struct TraceDocument {
    pub roots: Vec<TraceNode>,
    pub outcome: Option<TraceOutcome>,
}

impl TraceDocument {
    pub fn single(root: TraceNode) -> Self {
        Self {
            roots: vec![root],
            outcome: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub frames: usize,
    pub calls: usize,
    pub delegatecalls: usize,
    pub staticcalls: usize,
    pub creates: usize,
    pub reverts: usize,
    pub storage_writes: usize,
    pub gas_used: u64,
}

impl Stats {
    pub fn from_root(root: &TraceNode) -> Self {
        let mut stats = Self {
            gas_used: root.gas_used,
            ..Default::default()
        };
        accumulate_stats(root, &mut stats);
        stats
    }

    pub fn from_document(document: &TraceDocument) -> Self {
        let mut stats = Self::default();

        for root in &document.roots {
            let root_stats = Self::from_root(root);
            stats.frames += root_stats.frames;
            stats.calls += root_stats.calls;
            stats.delegatecalls += root_stats.delegatecalls;
            stats.staticcalls += root_stats.staticcalls;
            stats.creates += root_stats.creates;
            stats.reverts += root_stats.reverts;
            stats.storage_writes += root_stats.storage_writes;
            stats.gas_used += root_stats.gas_used;
        }

        stats
    }
}

fn accumulate_stats(node: &TraceNode, stats: &mut Stats) {
    stats.frames += 1;

    match node.kind {
        CallKind::Call => stats.calls += 1,
        CallKind::DelegateCall => stats.delegatecalls += 1,
        CallKind::StaticCall => stats.staticcalls += 1,
        CallKind::Create | CallKind::Create2 => stats.creates += 1,
        _ => {}
    }

    if node.status() == "REVERT" {
        stats.reverts += 1;
    }

    stats.storage_writes += node.storage_diff.len();

    for child in &node.calls {
        accumulate_stats(child, stats);
    }
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct SelectionDetails {
    pub trace_path: Vec<String>,
    pub storage: Vec<StorageChange>,
    pub children: usize,
    pub known_selector: Option<String>,
}

#[allow(dead_code)]
pub type SelectorMap = BTreeMap<String, String>;
