# evmtrace-tui

**Terminal-first EVM transaction trace explorer for developers, auditors, and security researchers.**

`evmtrace-tui` turns EVM transaction traces into an interactive terminal UI for exploring nested calls, reverts, gas usage, function selectors, and storage changes.

It is designed for fast local investigation of transaction execution without requiring an RPC connection, browser UI, telemetry, or a large debugging stack.

## What it does

`evmtrace-tui` reads a transaction trace and presents the execution as an interactive call tree.

You can:

* inspect nested `CALL`, `STATICCALL`, `DELEGATECALL`, `CALLCODE`, `CREATE`, `CREATE2`, and `SELFDESTRUCT` frames;
* move through the entire execution tree with the keyboard;
* inspect frame details, calldata size, value, gas, errors, and revert reasons;
* decode common function selectors automatically;
* load a Solidity or Foundry ABI for additional selector decoding;
* inspect frame-local storage changes;
* filter frames by address or selector;
* isolate reverted frames or frames that changed storage;
* navigate large traces without leaving the terminal.

The tool is intentionally **local and dependency-light**: it does not fetch transactions, call an RPC endpoint, send transactions, or collect telemetry.

## Features

### Interactive execution tree

The `TRACE` panel displays the transaction as a hierarchical execution tree with:

* call depth;
* call type;
* target address;
* function selector;
* execution status;
* gas used.

Nested calls remain visible as part of the same execution structure, making proxy paths and deeper call chains easier to inspect.

### Frame details

The `DETAILS` panel exposes the currently selected frame, including:

* call kind;
* status;
* sender and target;
* function selector;
* decoded function signature;
* value;
* gas used / gas limit;
* calldata size;
* child-call count;
* execution errors;
* revert reason.

### Storage changes

The `STORAGE` panel provides a trace-wide view of storage changes.

Frame-local storage changes can also be inspected from the details view.

The parser supports the normalized `storageDiff` extension used by this project:

```json
{
  "storageDiff": {
    "0x01": [
      "0x0000000000000000000000000000000000000000000000000000000000000000",
      "0x0000000000000000000000000000000000000000000000000000000000000012"
    ]
  }
}
```

A `storageDiffs` map keyed by frame id is also supported by the parser.

These storage fields are an extension of the call-trace model and are **not required fields in a standard Geth `callTracer` result**.

### Selector decoding

Common selectors are decoded automatically, including widely used ERC-20, ownership, and proxy functions.

For project-specific contracts, pass a Solidity or Foundry ABI:

```bash
cargo run -- examples/sample_trace.json --abi path/to/Contract.json
```

Both a raw ABI array and an artifact containing an `abi` array are supported.

Argument-level ABI decoding is intentionally outside the current scope and is planned for a future release.

## Input format

The primary input is a **Geth-style `callTracer` execution tree**.

A minimal trace looks like:

```json
{
  "type": "CALL",
  "from": "0x1111111111111111111111111111111111111111",
  "to": "0x2222222222222222222222222222222222222222",
  "value": "0x0",
  "gas": "0x5208",
  "gasUsed": "0x1f40",
  "input": "0xa9059cbb...",
  "output": "0x",
  "calls": [
    {
      "type": "DELEGATECALL",
      "from": "0x2222222222222222222222222222222222222222",
      "to": "0x3333333333333333333333333333333333333333",
      "gas": "0x4000",
      "gasUsed": "0x1200",
      "input": "0x..."
    }
  ]
}
```

A JSON-RPC wrapper is accepted as well:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "type": "CALL",
    "from": "0x...",
    "to": "0x...",
    "input": "0x...",
    "gas": "0x...",
    "gasUsed": "0x...",
    "calls": []
  }
}
```

The current parser is focused on the **call-frame tree model**. Opcode-level `structLogs` traces are not currently supported.

## Quick start

From the project directory:

```bash
cargo run -- examples/sample_trace.json
```

Build an optimized binary:

```bash
cargo build --release
```

Then:

```bash
./target/release/evmtrace-tui examples/sample_trace.json
```

With an ABI:

```bash
./target/release/evmtrace-tui \
  examples/sample_trace.json \
  --abi path/to/Contract.json
```

## Keyboard controls

### Navigation

| Key         | Action                                        |
| ----------- | --------------------------------------------- |
| `↑` / `k`   | Move to previous frame                        |
| `↓` / `j`   | Move to next frame                            |
| `←` / `h`   | Previous panel                                |
| `→` / `l`   | Next panel                                    |
| `Tab`       | Next panel                                    |
| `Shift+Tab` | Previous panel                                |
| `Home`      | First frame                                   |
| `End`       | Last frame                                    |
| `PageUp`    | Move up by 10 frames                          |
| `PageDown`  | Move down by 10 frames                        |
| `Enter`     | Open the Details panel for the selected frame |

The **physical arrow keys** are supported directly.

### Filtering

| Key         | Action                                        |
| ----------- | --------------------------------------------- |
| `/`         | Enter text filter mode                        |
| `Enter`     | Apply the filter                              |
| `Backspace` | Delete the previous character                 |
| `e`         | Toggle reverted frames only                   |
| `w`         | Toggle frames with storage changes            |
| `r`         | Clear all filters                             |
| `Ctrl+C`    | Clear the active filter / quit in normal mode |

Text filtering currently matches frame addresses and function selectors.

### General

| Key   | Action            |
| ----- | ----------------- |
| `?`   | Open help         |
| `Esc` | Close help / exit |
| `q`   | Exit              |

## Project structure

```text
evm-trace-tui/
├── examples/
│   └── sample_trace.json
├── src/
│   ├── app.rs
│   ├── main.rs
│   ├── model.rs
│   ├── parser.rs
│   ├── selector.rs
│   └── ui.rs
├── tests/
├── Cargo.toml
└── README.md
```

### Architecture

The application is intentionally split into small layers:

```text
Trace JSON
    │
    ▼
  parser
    │
    ▼
 TraceNode model
    │
    ├──────────────► selector decoding
    │
    ▼
    App state
    │
    ▼
 ratatui UI
```

The UI operates on a structured `TraceNode` model instead of raw JSON strings. This keeps parsing, application state, selector decoding, and rendering separated so additional trace providers can be added without rewriting the terminal interface.

## Development

Format:

```bash
cargo fmt
```

Check:

```bash
cargo check --all-targets
```

Run tests:

```bash
cargo test --all-targets
```

Run Clippy:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Build release:

```bash
cargo build --release
```

Before submitting changes, run the full verification set:

```bash
cargo fmt
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

## Design goals

`evmtrace-tui` is intentionally focused on one job:

> **Make EVM execution traces fast to inspect from a terminal.**

The project prioritizes:

* deterministic local behavior;
* a small dependency surface;
* keyboard-first navigation;
* structured trace data;
* readable execution trees;
* composable parsing and rendering components.

It does not attempt to replace full symbolic execution, fuzzing, transaction simulation, or an interactive debugger with live chain access.

## Roadmap

Planned directions include:

* Foundry `-vvvv` trace import;
* native `debug_traceTransaction` fetching with an explicitly supplied RPC endpoint;
* ABI argument decoding;
* tuple and array decoding;
* richer storage read/write timelines;
* balance and token-transfer deltas;
* call-depth heatmaps;
* gas flame views;
* subtree / frame export;
* advanced search by function signature, address, opcode, and revert reason.

The roadmap is intentionally incremental: the current trace explorer remains fully usable as a standalone local tool.

## License

MIT
