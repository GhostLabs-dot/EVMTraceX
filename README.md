# evmtrace-tui

[![CI](https://github.com/ghost-labs/evmtrace-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/ghost-labs/evmtrace-tui/actions/workflows/ci.yml)

Terminal-first EVM transaction trace explorer for inspecting nested calls, reverts, gas usage, selectors and storage changes without leaving the terminal.

## Features

- Geth-style `callTracer` JSON input (`result` wrapper is also accepted).
- Tree-shaped execution trace with depth, target, selector, status and gas.
- Built-in decoding for common ERC-20/proxy/ownership selectors.
- Optional Solidity/Foundry ABI JSON for additional selector decoding.
- Per-frame storage diff view using a small normalized `storageDiff` extension.
- Revert/error visibility.
- Interactive filters for reverts and storage-writing frames.
- Keyboard-first TUI built with `ratatui` + `crossterm`.
- No RPC access, no network calls, no telemetry.

## Quick start

```bash
cargo run -- examples/sample_trace.json
```

With an ABI:

```bash
cargo run -- examples/sample_trace.json --abi path/to/Counter.json
```

Build release binary:

```bash
cargo build --release
./target/release/evmtrace-tui examples/sample_trace.json
```

## Expected trace shape

The parser accepts a Geth `debug_traceTransaction` result produced with `callTracer`:

```json
{
  "type": "CALL",
  "from": "0x...",
  "to": "0x...",
  "input": "0xa9059cbb...",
  "gas": "0x...",
  "gasUsed": "0x...",
  "error": "execution reverted",
  "revertReason": "...",
  "calls": [
    { "type": "DELEGATECALL", "calls": [] }
  ]
}
```

For frame-local storage changes, this version supports:

```json
"storageDiff": {
  "0x01": ["0x00...", "0x12..."]
}
```

The root can also contain a `storageDiffs` map keyed by frame id.

## Controls

`↑/k` and `↓/j` move. `←/h` and `→/l` switch focus. `/` enters live text filtering; `Enter` applies and `Backspace` edits. `e` toggles revert-only. `w` toggles storage-write-only. `r` clears filters. `?` opens help. `q` or `Esc` exits.

## Roadmap

- Foundry `-vvvv` text trace importer.
- Native `debug_traceTransaction` fetch mode with an explicit RPC URL.
- ABI argument decoding, including tuples and arrays.
- Full storage read/write timeline.
- Balance and token transfer deltas.
- Call-depth heatmap and gas flame view.
- Export selected frame / subtree as JSON.
- Search by function signature, address, opcode or revert reason.

## Design principle

The UI depends on a stable `TraceNode` model rather than raw strings. That keeps the terminal view replaceable while allowing additional EVM trace providers to feed the same renderer.

## License

MIT
