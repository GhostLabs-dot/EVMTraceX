# EVM Trace TUI

**Terminal-first EVM transaction trace explorer for developers, auditors, and security researchers.**

`evmtrace-tui` turns EVM transaction traces into an interactive terminal UI for exploring nested calls, reverts, gas usage, function selectors, and storage changes.

It is designed for fast, local investigation of EVM execution from trace files, Foundry test output, or an explicitly supplied JSON-RPC endpoint.

---

## What it does

`evmtrace-tui` parses supported EVM execution traces into a structured call tree and presents them through a keyboard-driven terminal interface.

You can:

* inspect nested `CALL`, `STATICCALL`, `DELEGATECALL`, `CALLCODE`, `CREATE`, `CREATE2`, and `SELFDESTRUCT` frames;
* move through the execution tree with the keyboard;
* inspect callers, targets, selectors, calldata size, value, gas, errors, and revert reasons;
* decode common function selectors automatically;
* load a Solidity or Foundry ABI for additional selector decoding;
* inspect frame-local and trace-wide storage changes;
* filter frames by address, selector, label, or other frame text;
* isolate reverted frames;
* isolate frames with storage changes;
* inspect multiple independent Foundry test traces without merging them into one synthetic transaction;
* fetch a completed transaction trace directly through `debug_traceTransaction` when an RPC endpoint is explicitly supplied.

The application is terminal-first and local by default. Network access is used only when the RPC mode is explicitly selected.

---

## Features

### Interactive execution tree

The `TRACE` panel displays the parsed execution structure as a hierarchical call tree.

Supported frame types include:

* `CALL`
* `STATICCALL`
* `DELEGATECALL`
* `CALLCODE`
* `CREATE`
* `CREATE2`
* `SELFDESTRUCT`
* `TEST`
* `CHEATCODE`
* `UNKNOWN`

For each frame, the interface can expose:

* call depth;
* call type;
* sender;
* target;
* function selector;
* decoded function signature;
* execution status;
* gas used;
* gas limit when available;
* calldata size;
* value;
* child-call count;
* execution errors;
* revert reasons;
* frame-local storage changes.

Nested calls remain attached to their parent frame, preserving execution structure.

---

### Foundry `-vvvv` traces

Foundry execution traces can be loaded directly from the text produced by:

```bash
forge test -vvvv > foundry-trace.txt
```
For storage-change output, use:

```bash
forge test -vvvvv > foundry-trace.txt
```

Then open the trace:

```bash
./target/release/evmtrace-tui --foundry foundry-trace.txt
```

The parser understands Foundry trace sections beginning with:

```text
Traces:
```

and preserves individual traced tests as separate roots.

For example:

```text
Foundry output
    │
    ├── CounterTest::test_First()
    │     ├── Counter::increment()
    │     └── Counter::number() [staticcall]
    │
    └── CounterTest::test_Second()
          └── Counter::balance()
```

This prevents gas totals and execution structure from different tests being merged into one artificial root.

Foundry labels such as:

```text
CounterTest::test_Increment()
Counter::increment()
Token::balanceOf(...)
```

are retained in the trace model and shown in the UI.

Recognized Foundry markers such as:

```text
[staticcall]
[delegatecall]
[callcode]
```

are mapped to the corresponding `CallKind`.

Return and revert markers are associated with the execution frame that produced them.

#### Foundry-specific behavior and limitations

Foundry text traces are normalized conservatively rather than reconstructed into fields that are not explicitly present.

Foundry `VM::...` cheatcodes are represented as `CHEATCODE` frames. The Foundry test invocation root is represented as `TEST`. These frames are kept separate from actual EVM call kinds.

When a Foundry label contains an explicit 20-byte address before `::`, that address is preserved as the frame target, and a known parent target may be used as the child caller.

Available frame gas is shown as gas used. A gas limit is not inferred from the text trace.

Raw calldata is not fabricated from a human-readable function label. When calldata is absent, selector and input size are shown as unavailable rather than zero.

The original human-readable Foundry label is preserved.

Foundry `-vvvvv` storage-change lines are parsed into the normalized `storage_diff` representation.

---

### Geth-style `callTracer` JSON

The primary structured input format is a Geth-style call-frame tree.

Example:

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

A JSON-RPC wrapper containing the trace under `result` is accepted as well:

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

The parser is intentionally focused on the call-frame tree model.

Opcode-level `structLogs` traces are not currently supported.

---

### RPC transaction tracing

A completed transaction can be traced directly through a supplied JSON-RPC endpoint:

```bash
./target/release/evmtrace-tui \
  --tx 0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --rpc https://your-rpc-endpoint
```

The application requests:

```text
debug_traceTransaction
```

using the Geth-style:

```json
{
  "tracer": "callTracer"
}
```

The transaction hash must be a valid 32-byte hexadecimal value beginning with `0x`.

The supplied RPC endpoint must support the required debug/tracing method.

The application does not contact an RPC endpoint when a local trace file or Foundry trace file is used.

---

### Selector decoding

Function selectors are derived from the first four bytes of calldata.

For example:

```text
0xa9059cbb
```

is recognized as:

```text
transfer(address,uint256)
```

The built-in selector table currently includes commonly encountered ERC-20, ownership, and proxy functions such as:

```text
transfer(address,uint256)
transferFrom(address,address,uint256)
approve(address,uint256)
balanceOf(address)
allowance(address,address)
totalSupply()
name()
symbol()
decimals()
mint(address,uint256)
safeTransferFrom(address,address,uint256)
safeTransferFrom(address,address,uint256,bytes)
upgradeTo(address)
upgradeToAndCall(address,bytes)
admin()
implementation()
proxiableUUID()
renounceOwnership()
owner()
transferOwnership(address)
```

For project-specific contracts, an ABI can be supplied:

```bash
./target/release/evmtrace-tui \
  examples/sample_trace.json \
  --abi path/to/Contract.json
```

Both of the following are supported:

#### Raw ABI array

```json
[
  {
    "type": "function",
    "name": "transfer",
    "inputs": [
      {
        "name": "to",
        "type": "address"
      },
      {
        "name": "amount",
        "type": "uint256"
      }
    ]
  }
]
```

#### Solidity / Foundry artifact

```json
{
  "abi": [
    {
      "type": "function",
      "name": "transfer",
      "inputs": [
        {
          "name": "to",
          "type": "address"
        },
        {
          "name": "amount",
          "type": "uint256"
        }
      ]
    }
  ]
}
```

Canonical ABI type handling also supports tuple definitions when constructing function signatures.

#### Current ABI boundary

The current implementation decodes function selectors into signatures.

It does **not** yet decode arbitrary calldata arguments into typed values.

---

### Storage changes

The `STORAGE` panel provides a trace-wide view of storage modifications.

Frame-local storage changes are also displayed in the `DETAILS` panel.

The normalized trace model supports the project's `storageDiff` extension:

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

The parser also accepts the equivalent object representation:

```json
{
  "storageDiff": {
    "0x01": {
      "before": "0x0000000000000000000000000000000000000000000000000000000000000000",
      "after": "0x0000000000000000000000000000000000000000000000000000000000000012"
    }
  }
}
```

A top-level `storageDiffs` map keyed by frame ID is supported as well.

These storage fields are extensions of the normalized trace model and are **not required fields in a standard Geth `callTracer` response**.

---

### Revert and error visibility

A frame is displayed as:

```text
SUCCESS
```

when neither an execution error nor a revert reason is present.

A frame is displayed as:

```text
REVERT
```

when either `error` or `revertReason` is populated.

When a revert reason is available, it is preserved and displayed in the `DETAILS` panel.

Foundry return and revert markers are mapped into the same normalized model so the UI can inspect them consistently alongside structured JSON traces.

---

## Interface

The interface is intentionally compact and terminal-first.

```text
┌───────────────────────────────────────────────────────────────────────┐
│ evmtrace-tui │ 8 frames │ 1 reverts │ 1 storage writes │ 1 roots     │
├───────────────────────────────────────────────────────────────────────┤
│ TRACE                 STORAGE                 DETAILS                 │
├───────────────────────────────────────────────────────────────────────┤
│                                                                       │
│ ▶ CALL          0x2222…1111   0x........  SUCCESS   gas  12,450      │
│   ├─ CALL      0x3333…4444   0xa9059cbb  SUCCESS   gas   8,192      │
│   ├─ CALL      0x5555…6666   0x........  REVERT    gas   4,096      │
│   └─ DELEGATECALL 0x6666…77  0x8da5cb5b  SUCCESS   gas   7,840      │
│       └─ STATICCALL 0x7777…  0x70a08231  SUCCESS   gas   2,048      │
│                                                                       │
├───────────────────────────────────────────────────────────────────────┤
│ ↑/k ↓/j move · ←/h →/l panel · / filter · e errors · w writes        │
│ Frame 1/8 · TRACE · 1 roots · Ready                                  │
└───────────────────────────────────────────────────────────────────────┘
```

The application has three primary panels:

### `TRACE`

The execution tree and frame navigation view.

### `STORAGE`

Trace-wide storage modifications associated with execution frames.

### `DETAILS`

Detailed information about the currently selected frame.

---

## Keyboard controls

### Navigation

| Key         | Action                 |
| ----------- | ---------------------- |
| `↑` / `k`   | Move to previous frame |
| `↓` / `j`   | Move to next frame     |
| `←` / `h`   | Previous panel         |
| `→` / `l`   | Next panel             |
| `Tab`       | Next panel             |
| `Shift+Tab` | Previous panel         |
| `Home`      | First frame            |
| `End`       | Last frame             |
| `PageUp`    | Move up by 10 frames   |
| `PageDown`  | Move down by 10 frames |
| `Enter`     | Open the Details panel |

Physical arrow keys are supported directly.

---

### Filtering

Use `/` to enter text-filter mode.

| Key         | Action                               |
| ----------- | ------------------------------------ |
| `/`         | Enter text filter mode               |
| `Enter`     | Apply the filter                     |
| `Backspace` | Remove the previous filter character |
| `e`         | Toggle reverted frames only          |
| `w`         | Toggle frames with storage changes   |
| `r`         | Clear all filters                    |
| `Esc`       | Cancel text-filter input             |
| `Ctrl+C`    | Clear the active text filter         |

The text filter is matched against frame display names, sender addresses, target addresses, and selectors.

Filtering operates on the normalized trace model rather than the original input text.

---

### General

| Key      | Action                        |
| -------- | ----------------------------- |
| `?`      | Open help                     |
| `Esc`    | Close help / exit normal mode |
| `q`      | Quit                          |
| `Ctrl+C` | Quit in normal mode           |

---

## Quick start

Clone or enter the project directory and build it:

```bash
cargo build --release
```

Run the included example:

```bash
./target/release/evmtrace-tui examples/sample_trace.json
```

Or directly through Cargo:

```bash
cargo run -- examples/sample_trace.json
```

With an ABI:

```bash
./target/release/evmtrace-tui \
  examples/sample_trace.json \
  --abi path/to/Contract.json
```

With a Foundry trace:

```bash
forge test -vvvv > foundry-trace.txt

./target/release/evmtrace-tui \
  --foundry foundry-trace.txt
```

With a Foundry trace and ABI:

```bash
./target/release/evmtrace-tui \
  --foundry foundry-trace.txt \
  --abi path/to/Contract.json
```

With a remote transaction:

```bash
./target/release/evmtrace-tui \
  --tx 0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --rpc https://your-rpc-endpoint
```

---

## Trace document model

All supported inputs are normalized into a common document model.

```text
TraceDocument
│
├── root
│   └── TraceNode
│       ├── TraceNode
│       ├── TraceNode
│       └── ...
│
└── root
    └── TraceNode
        └── ...
```

A normal Geth or RPC transaction contains one root and no external test outcome metadata.

A Foundry file may contain multiple roots, allowing separate traced tests to remain independent. When Foundry reports a final `Suite result`, the normalized document preserves the aggregate pass/fail outcome separately from individual EVM frame status.

Each `TraceNode` contains the execution information needed by the application:

```text
TraceNode
├── id
├── depth
├── kind
├── label
├── from
├── to
├── input
├── output
├── value
├── gas
├── gas_used
├── error
├── revert_reason
├── storage_diff
└── calls
```

This normalized representation keeps parsing separate from application state and rendering.

---

## Architecture

The project is intentionally split into small, focused layers:

```text
                         INPUT SOURCES
                              │
               ┌──────────────┼──────────────┐
               │              │              │
               ▼              ▼              ▼
           Geth JSON       Foundry       JSON-RPC
                              │              │
               └──────────────┼──────────────┘
                              ▼
                       ┌─────────────┐
                       │  parser.rs  │
                       └──────┬──────┘
                              │
                              ▼
                       ┌─────────────┐
                       │  model.rs   │
                       │             │
                       │ Document    │
                       │ TraceNode   │
                       │ Stats       │
                       └──────┬──────┘
                              │
                 ┌────────────┼────────────┐
                 ▼            ▼            ▼
            selector.rs    app.rs        filters
                 │            │
                 └────────────┼───────────┘
                              ▼
                         ┌──────────┐
                         │  ui.rs   │
                         │ ratatui  │
                         └──────────┘
```

### `parser.rs`

Handles:

* local trace-file loading;
* Geth call-frame parsing;
* JSON-RPC wrapper parsing;
* `debug_traceTransaction`;
* Foundry `-vvvv` parsing;
* ABI loading;
* storage-diff parsing;
* normalization into the internal model.

### `model.rs`

Defines:

* `CallKind`;
* `StorageChange`;
* `TraceNode`;
* `TraceDocument`;
* `TraceOutcome`;
* `Stats`.

### `app.rs`

Maintains:

* frame selection;
* panel selection;
* text filters;
* revert filters;
* storage filters;
* selector mappings;
* help state;
* input mode.

### `selector.rs`

Provides:

* Keccak-256 selector generation;
* built-in selector recognition.

### `ui.rs`

Renders:

* header statistics;
* execution trace;
* storage view;
* frame details;
* gas information;
* help;
* keyboard shortcuts.

### `main.rs`

Provides:

* CLI parsing;
* input-source selection;
* terminal lifecycle;
* event handling;
* keyboard controls;
* integration tests.

---

## Project structure

The project itself contains:

```text
evm-trace-tui/
├── .github/
│   └── workflows/
│       └── ci.yml
├── examples/
│   └── sample_trace.json
├── src/
│   ├── app.rs
│   ├── main.rs
│   ├── model.rs
│   ├── parser.rs
│   ├── selector.rs
│   └── ui.rs
├── .gitignore
├── CHANGELOG.md
├── CONTRIBUTING.md
├── Cargo.toml
├── LICENSE
├── README.md
└── rustfmt.toml
```

The current automated tests are located in `src/main.rs`; there is no separate `tests/` directory.

---

## Example fixture

The repository ships with:

```text
examples/sample_trace.json
```

The fixture contains representative execution data including:

* a root `CALL`;
* nested calls;
* an ERC-20-style selector;
* a reverting frame;
* a `DELEGATECALL`;
* a nested `STATICCALL`;
* a storage change.

Run it with:

```bash
cargo run -- examples/sample_trace.json
```

The fixture is intended as a deterministic local starting point for exploring the UI and exercising parser behavior.

---

## Development

The project uses stable Rust and a small dependency footprint built around:

* `ratatui` for the terminal UI;
* `crossterm` for terminal input/output;
* `clap` for command-line parsing;
* `serde` and `serde_json` for structured trace and ABI data;
* `reqwest` for optional RPC access;
* `sha3` and `hex` for selector handling;
* `anyhow` for error propagation.

### Format

```bash
cargo fmt
```

### Check

```bash
cargo check --all-targets
```

### Tests

```bash
cargo test --all-targets
```

### Clippy

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Release build

```bash
cargo build --release
```

### Full verification

Before submitting changes, run:

```bash
cargo fmt
cargo fmt --all -- --check
git diff --check
cargo check --all-targets --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
cargo package --locked
cargo audit
```

---

## CI

The project CI verifies:

```bash
cargo fmt --all -- --check
cargo check --all-targets --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
cargo package --locked
cargo audit
```

The CI configuration is scoped to the TUI project and its workflow changes.

---

## Design goals

`evmtrace-tui` is intentionally focused on one job:

> **Make EVM execution traces fast to inspect from a terminal.**

The project prioritizes:

* local-first operation;
* keyboard-first navigation;
* structured trace data;
* readable nested execution trees;
* explicit source handling;
* predictable behavior;
* a small dependency surface;
* separation between parsing, application state, and rendering.

It intentionally avoids turning the project into a full EVM framework.

---

## What it is not

`evmtrace-tui` is a trace explorer, not a vulnerability scanner.

It does not currently provide:

* symbolic execution;
* fuzzing;
* exploit generation;
* transaction simulation;
* state mutation;
* live transaction debugging;
* opcode-level `structLogs` analysis;
* automatic vulnerability classification;
* arbitrary ABI argument decoding;
* chain-wide transaction indexing;
* token-transfer or balance analytics.

The application presents execution evidence for human inspection rather than automatically deciding what that evidence means.

---

## Current limitations

### Opcode-level traces

`structLogs` and instruction-by-instruction execution are not currently parsed.

### ABI arguments

Function selectors can be mapped to function signatures, but arbitrary calldata arguments are not decoded into typed values.

### Foundry metadata

Foundry text traces do not necessarily contain the same structured fields available in JSON call traces.

### RPC requirements

RPC mode depends on an endpoint that exposes `debug_traceTransaction` with `callTracer` support.

### Trace scope

The current model focuses on execution call frames rather than full blockchain transaction metadata.

---

## Security and privacy

Local JSON and Foundry files are processed locally.

When RPC mode is used, the transaction hash is sent to the explicitly supplied RPC endpoint.

The application does not embed a default public RPC provider.

Users should treat transaction hashes and RPC endpoints according to the privacy and logging policies of the provider they choose.

---

## Contributing

Contributions are welcome.

When extending parser support, new input formats should be normalized into the existing trace model rather than adding format-specific rendering logic.

Parser and semantic changes should include focused tests or representative fixtures where practical.

See `CONTRIBUTING.md` for the project contribution guidelines.

---

## Author and project

**Created and maintained by Ghost Labs.**

**Founded by Genadi Avetisov.**

The project is developed as part of Ghost Labs' broader work in EVM tooling, runtime analysis, and security research.

---

## License

MIT License.

Copyright (c) 2026 Ghost Labs.

See [`LICENSE`](LICENSE) for the full license text.

---

## Project status

The current package version is `0.1.0`.

The project is intentionally kept focused: parse execution traces, preserve their structure, and make them easy to investigate from a terminal.

```text
Geth callTracer JSON ─────┐
                          │
Foundry forge test -vvvv ─┼──► normalized trace model ──► terminal UI
                          │
debug_traceTransaction ───┘
```

**Inspect the trace. Preserve the structure. Find the interesting frame.**
