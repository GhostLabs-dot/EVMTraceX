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
        (s.len() >= 8).then(|| &s[..8])
    }

    pub fn input_bytes_len(&self) -> usize {
        let s = self.input.strip_prefix("0x").unwrap_or(&self.input);
        s.len() / 2
    }

    pub fn flatten(&self, out: &mut Vec<TraceRow>) {
        out.push(TraceRow {
            id: self.id,
            depth: self.depth,
            kind: self.kind.clone(),
            from: self.from.clone(),
            to: self.to.clone(),
            gas_used: self.gas_used,
            status: self.status(),
        });
        for child in &self.calls {
            child.flatten(out);
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct TraceRow {
    pub id: usize,
    pub depth: usize,
    pub kind: CallKind,
    pub from: String,
    pub to: String,
    pub gas_used: u64,
    pub status: &'static str,
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
        let mut rows = Vec::new();
        root.flatten(&mut rows);
        let mut s = Self {
            frames: rows.len(),
            gas_used: root.gas_used,
            ..Default::default()
        };
        for r in rows {
            match r.kind {
                CallKind::Call => s.calls += 1,
                CallKind::DelegateCall => s.delegatecalls += 1,
                CallKind::StaticCall => s.staticcalls += 1,
                CallKind::Create | CallKind::Create2 => s.creates += 1,
                _ => {}
            }
            if r.status == "REVERT" {
                s.reverts += 1;
            }
        }
        s.storage_writes = count_storage_writes(root);
        s
    }
}

fn count_storage_writes(node: &TraceNode) -> usize {
    node.storage_diff.len() + node.calls.iter().map(count_storage_writes).sum::<usize>()
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
