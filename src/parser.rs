use crate::model::{CallKind, StorageChange, TraceDocument, TraceNode, TraceOutcome};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path, time::Duration};

pub fn load_trace(path: &Path) -> Result<TraceDocument> {
    let raw = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(TraceDocument::single(parse_trace(&raw)?))
}

pub fn load_foundry_trace(path: &Path) -> Result<TraceDocument> {
    let raw = fs::read_to_string(path).with_context(|| format!("failed to read Foundry trace {}", path.display()))?;
    parse_foundry_trace(&raw)
}

pub fn load_trace_rpc(rpc_url: &str, tx_hash: &str) -> Result<TraceDocument> {
    let tx_hash = tx_hash.trim();

    let Some(raw_hash) = tx_hash.strip_prefix("0x") else {
        bail!("transaction hash must start with 0x");
    };

    if raw_hash.len() != 64 || !raw_hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("transaction hash must be a 32-byte hex value");
    }

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "debug_traceTransaction",
        "params": [
            tx_hash,
            {
                "tracer": "callTracer"
            }
        ]
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("failed to create HTTP client")?;

    let response = client
        .post(rpc_url)
        .json(&payload)
        .send()
        .context("failed to query RPC endpoint")?;

    let status = response.status();

    if !status.is_success() {
        bail!("RPC request failed with HTTP {}", status);
    }

    let body = response.text().context("failed to read RPC response body")?;

    let value: Value = serde_json::from_str(&body).context("RPC returned invalid JSON")?;

    if let Some(error) = value.get("error") {
        bail!("RPC error: {}", error);
    }

    Ok(TraceDocument::single(parse_trace(&body)?))
}

pub fn parse_trace(raw: &str) -> Result<TraceNode> {
    let value: Value = serde_json::from_str(raw).context("invalid JSON trace")?;
    let root = value.get("result").unwrap_or(&value);
    if !root.is_object() {
        bail!("trace root must be a JSON object");
    }

    let mut next_id = 0usize;
    let mut node = parse_node(root, 0, &mut next_id)?;
    attach_storage_extensions(root, &mut node);
    Ok(node)
}

pub fn parse_foundry_trace(raw: &str) -> Result<TraceDocument> {
    let mut roots = Vec::new();
    let mut stack = Vec::<TraceNode>::new();
    let mut next_id = 0usize;
    let mut in_traces = false;
    let mut outcome = None;

    for line in raw.lines() {
        let trimmed = line.trim_start();

        if trimmed == "Traces:" {
            flush_foundry_stack(&mut stack, &mut roots);
            in_traces = true;
            continue;
        }

        if trimmed.starts_with("Suite result:") {
            outcome = parse_foundry_outcome(trimmed);
            flush_foundry_stack(&mut stack, &mut roots);
            in_traces = false;
            continue;
        }

        if !in_traces {
            continue;
        }

        if let Some(change) = parse_foundry_storage_change(line) {
            if let Some(node) = stack.last_mut() {
                node.storage_diff.push(change);
            }
            continue;
        }

        if let Some((depth, gas_used, label, kind)) = parse_foundry_call(line) {
            if depth > stack.len() {
                bail!("invalid Foundry trace indentation near: {}", line.trim());
            }

            while stack.len() > depth {
                let child = stack.pop().expect("stack is non-empty");
                if let Some(parent) = stack.last_mut() {
                    parent.calls.push(child);
                } else {
                    roots.push(child);
                }
            }

            let is_vm = label.starts_with("VM::");
            let to = if is_vm {
                String::new()
            } else {
                foundry_target_from_label(&label)
            };
            let from = if is_vm {
                String::new()
            } else {
                stack.last().map(|parent| parent.to.clone()).unwrap_or_default()
            };

            stack.push(TraceNode {
                id: next_id,
                depth,
                kind,
                label: Some(label),
                from,
                to,
                input: String::new(),
                output: String::new(),
                value: String::new(),
                gas: 0,
                gas_used,
                error: None,
                revert_reason: None,
                storage_diff: Vec::new(),
                calls: Vec::new(),
            });

            next_id += 1;
            continue;
        }

        if let Some((status, detail)) = parse_foundry_result(line) {
            let depth = foundry_result_depth(line);

            if let Some(node) = stack.get_mut(depth) {
                match status.to_ascii_lowercase().as_str() {
                    "stop" => {}
                    "return" => {
                        node.output = detail;
                    }
                    "revert" => {
                        node.error = Some("reverted".into());
                        if !detail.is_empty() {
                            node.revert_reason = Some(detail);
                        }
                    }
                    _ => {
                        node.error = Some(status);
                        if !detail.is_empty() {
                            node.revert_reason = Some(detail);
                        }
                    }
                }
            }
        }
    }

    flush_foundry_stack(&mut stack, &mut roots);

    if roots.is_empty() {
        bail!("no Foundry -vvvv traces found");
    }

    Ok(TraceDocument { roots, outcome })
}

fn flush_foundry_stack(stack: &mut Vec<TraceNode>, roots: &mut Vec<TraceNode>) {
    while let Some(child) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.calls.push(child);
        } else {
            roots.push(child);
        }
    }
}

fn parse_foundry_call(line: &str) -> Option<(usize, u64, String, CallKind)> {
    let open = line.find('[')?;
    let close = line[open + 1..].find(']')? + open + 1;

    let gas_text = line[open + 1..close].trim();
    if gas_text.is_empty() || !gas_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    let gas_used = gas_text.parse().ok()?;
    let depth = if open <= 2 {
        0
    } else {
        1 + line[..open].chars().filter(|ch| *ch == '│').count()
    };

    let payload = line[close + 1..].trim();
    if payload.is_empty() {
        return None;
    }

    if payload.starts_with('←') {
        return None;
    }

    let lower = payload.to_ascii_lowercase();

    if payload.starts_with("VM::") {
        return Some((depth, gas_used, payload.to_string(), CallKind::Unknown));
    }

    if let Some(label) = payload.strip_suffix("[staticcall]") {
        return Some((depth, gas_used, label.trim().to_string(), CallKind::StaticCall));
    }

    if let Some(label) = payload.strip_suffix("[delegatecall]") {
        return Some((depth, gas_used, label.trim().to_string(), CallKind::DelegateCall));
    }

    if let Some(label) = payload.strip_suffix("[callcode]") {
        return Some((depth, gas_used, label.trim().to_string(), CallKind::CallCode));
    }

    let kind = if depth == 0 {
        CallKind::Unknown
    } else if payload.starts_with("→ new ") {
        CallKind::Create
    } else if lower.contains("selfdestruct") {
        CallKind::SelfDestruct
    } else {
        CallKind::Call
    };

    Some((depth, gas_used, payload.to_string(), kind))
}

fn parse_foundry_result(line: &str) -> Option<(String, String)> {
    let marker = line.split_once('←')?.1.trim();

    if let Some(rest) = marker.strip_prefix('[') {
        let close = rest.find(']')?;
        let status = rest[..close].trim().to_string();
        let detail = rest[close + 1..].trim().to_string();
        return Some((status, detail));
    }

    if marker.is_empty() {
        return None;
    }

    Some(("return".into(), marker.to_string()))
}

fn foundry_result_depth(line: &str) -> usize {
    line.split_once('←')
        .map(|(prefix, _)| prefix.chars().filter(|ch| *ch == '│').count())
        .unwrap_or(0)
}

fn parse_foundry_outcome(line: &str) -> Option<TraceOutcome> {
    let summary = line.strip_prefix("Suite result:")?.trim_start();

    if summary.starts_with("FAILED") {
        Some(TraceOutcome::Failed)
    } else if summary.starts_with("ok.") {
        Some(TraceOutcome::Passed)
    } else {
        None
    }
}

fn foundry_target_from_label(label: &str) -> String {
    let Some((target, _)) = label.split_once("::") else {
        return String::new();
    };

    let target = target.trim();
    let Some(hex) = target.strip_prefix("0x") else {
        return String::new();
    };

    if hex.len() != 40 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return String::new();
    }

    target.to_string()
}

fn parse_foundry_storage_change(line: &str) -> Option<StorageChange> {
    let marker = line.split_once('@')?.1.trim();
    let (slot_part, values) = marker.split_once(':')?;

    let slot = slot_part
        .split_once('(')
        .map(|(slot, _)| slot)
        .unwrap_or(slot_part)
        .trim();

    let (before, after) = values.split_once('→').or_else(|| values.split_once("->"))?;

    let before = before.trim();
    let after = after.trim();

    if slot.is_empty() || before.is_empty() || after.is_empty() {
        return None;
    }

    Some(StorageChange {
        slot: slot.to_string(),
        before: before.to_string(),
        after: after.to_string(),
    })
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
        label: None,
        from: str_field(value, "from"),
        to: {
            let to = str_field(value, "to");
            if to.is_empty() {
                str_field(value, "address")
            } else {
                to
            }
        },
        input: {
            let input = str_field(value, "input");
            if input.is_empty() {
                str_field(value, "data")
            } else {
                input
            }
        },
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
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    obj.iter()
        .filter_map(|(slot, entry)| {
            if let Some(pair) = entry.as_array() {
                let before = pair.first()?.as_str()?.to_string();
                let after = pair.get(1)?.as_str()?.to_string();
                return Some(crate::model::StorageChange {
                    slot: slot.clone(),
                    before,
                    after,
                });
            }
            let before = entry.get("before")?.as_str()?.to_string();
            let after = entry.get("after")?.as_str()?.to_string();
            Some(crate::model::StorageChange {
                slot: slot.clone(),
                before,
                after,
            })
        })
        .collect()
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
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let raw = fs::read_to_string(path).with_context(|| format!("failed to read ABI {}", path.display()))?;
    let value: Value = serde_json::from_str(&raw).context("invalid ABI JSON")?;
    let items = if let Some(items) = value.as_array() {
        items
    } else {
        value
            .get("abi")
            .and_then(Value::as_array)
            .context("ABI must be a JSON array or an object containing an 'abi' array")?
    };
    let mut out = BTreeMap::new();
    for item in items {
        if item.get("type").and_then(Value::as_str) != Some("function") {
            continue;
        }
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(inputs) = item.get("inputs").and_then(Value::as_array) else {
            continue;
        };
        let types = inputs.iter().filter_map(canonical_abi_type).collect::<Vec<_>>();
        if types.len() != inputs.len() {
            continue;
        }
        let signature = format!("{}({})", name, types.join(","));
        let selector = crate::selector::selector4(&signature);
        out.insert(selector, signature);
    }
    Ok(out)
}

fn canonical_abi_type(input: &Value) -> Option<String> {
    let ty = input.get("type")?.as_str()?;
    if let Some(suffix) = ty.strip_prefix("tuple") {
        let components = input.get("components")?.as_array()?;
        let inner = components.iter().map(canonical_abi_type).collect::<Option<Vec<_>>>()?;
        Some(format!("({}){}", inner.join(","), suffix))
    } else {
        Some(ty.to_string())
    }
}
