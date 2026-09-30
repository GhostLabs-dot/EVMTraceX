use crate::model::{CallKind, TraceNode};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

pub fn load_trace(path: &Path) -> Result<TraceNode> {
    let raw = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_trace(&raw)
}

pub fn parse_trace(raw: &str) -> Result<TraceNode> {
    let value: Value = serde_json::from_str(raw).context("invalid JSON trace")?;
    let root = if let Some(result) = value.get("result") { result } else { &value };
    let mut next_id = 0usize;
    let mut node = parse_node(root, 0, &mut next_id)?;
    attach_storage_extensions(&value, &mut node);
    Ok(node)
}

fn parse_node(value: &Value, depth: usize, next_id: &mut usize) -> Result<TraceNode> {
    let id = *next_id;
    *next_id += 1;
    let kind = parse_kind(value.get("type").and_then(Value::as_str).unwrap_or("CALL"));
    let gas = parse_u64(value.get("gas").or_else(|| value.get("gasLimit")));
    let gas_used = parse_u64(value.get("gasUsed").or_else(|| value.get("gas_used")));
    let mut node = TraceNode {
        id,
        depth,
        kind,
        from: str_field(value, "from"),
        to: str_field(value, "to").or_else(|| str_field(value, "address")),
        input: str_field(value, "input").or_else(|| str_field(value, "data")),
        output: str_field(value, "output"),
        value: str_field(value, "value"),
        gas,
        gas_used,
        error: optional_string(value, "error"),
        revert_reason: optional_string(value, "revertReason"),
        storage_diff: parse_storage_diff(value.get("storageDiff").or_else(|| value.get("storage_diff"))),
        calls: Vec::new(),
    };
    if let Some(calls) = value.get("calls").and_then(Value::as_array) {
        for child in calls {
            node.calls.push(parse_node(child, depth + 1, next_id)?);
        }
    }
    Ok(node)
}

fn attach_storage_extensions(root_value: &Value, root: &mut TraceNode) {
    if let Some(map) = root_value.get("storageDiffs").and_then(Value::as_object) {
        apply_storage_map(map, root);
    }
}

fn apply_storage_map(map: &serde_json::Map<String, Value>, node: &mut TraceNode) {
    for (key, value) in map {
        if let Ok(id) = key.parse::<usize>() {
            if id == node.id {
                node.storage_diff = parse_storage_diff(Some(value));
            }
        }
    }
    for child in &mut node.calls {
        apply_storage_map(map, child);
    }
}

fn parse_kind(raw: &str) -> CallKind {
    match raw.to_ascii_uppercase().as_str() {
        "CALL" => CallKind::Call,
        "STATICCALL" => CallKind::StaticCall,
        "DELEGATECALL" => CallKind::DelegateCall,
        "CALLCODE" => CallKind::CallCode,
        "CREATE" => CallKind::Create,
        "CREATE2" => CallKind::Create2,
        "SELFDESTRUCT" => CallKind::SelfDestruct,
        _ => CallKind::Unknown,
    }
}

fn parse_storage_diff(value: Option<&Value>) -> Vec<crate::model::StorageChange> {
    let Some(value) = value else { return Vec::new(); };
    let Some(obj) = value.as_object() else { return Vec::new(); };
    obj.iter().filter_map(|(slot, entry)| {
        if let Some(pair) = entry.as_array() {
            let before = pair.get(0)?.as_str()?.to_string();
            let after = pair.get(1)?.as_str()?.to_string();
            return Some(crate::model::StorageChange { slot: slot.clone(), before, after });
        }
        let before = entry.get("before")?.as_str()?.to_string();
        let after = entry.get("after")?.as_str()?.to_string();
        Some(crate::model::StorageChange { slot: slot.clone(), before, after })
    }).collect()
}

fn parse_u64(value: Option<&Value>) -> u64 {
    match value {
        Some(Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(Value::String(s)) => parse_hex_or_dec(s),
        _ => 0,
    }
}

fn parse_hex_or_dec(s: &str) -> u64 {
    if let Some(v) = s.strip_prefix("0x") {
        u64::from_str_radix(v, 16).unwrap_or(0)
    } else {
        s.parse().unwrap_or(0)
    }
}

fn str_field(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn optional_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(ToOwned::to_owned)
}

pub fn load_selectors(path: Option<&Path>) -> Result<BTreeMap<String, String>> {
    let Some(path) = path else { return Ok(BTreeMap::new()); };
    let raw = fs::read_to_string(path).with_context(|| format!("failed to read ABI {}", path.display()))?;
    let value: Value = serde_json::from_str(&raw).context("invalid ABI JSON")?;
    let items = value.as_array().context("ABI must be a JSON array")?;
    let mut out = BTreeMap::new();
    for item in items {
        if item.get("type").and_then(Value::as_str) != Some("function") { continue; }
        let Some(name) = item.get("name").and_then(Value::as_str) else { continue; };
        let Some(inputs) = item.get("inputs").and_then(Value::as_array) else { continue; };
        let types = inputs.iter().filter_map(canonical_abi_type).collect::<Vec<_>>();
        if types.len() != inputs.len() { continue; }
        let signature = format!("{}({})", name, types.join(","));
        let selector = crate::selector::selector4(&signature);
        out.insert(selector, signature);
    }
    Ok(out)
}

fn canonical_abi_type(input: &Value) -> Option<String> {
    let ty = input.get("type")?.as_str()?;
    if ty.starts_with("tuple") {
        let components = input.get("components")?.as_array()?;
        let inner = components.iter().map(canonical_abi_type).collect::<Option<Vec<_>>>()?;
        let suffix = &ty["tuple".len()..];
        Some(format!("({}){}", inner.join(","), suffix))
    } else {
        Some(ty.to_string())
    }
}
