# Changelog

## 0.2.0 - EVMTraceX branding and distribution

- Rename the executable to `evmtracex` to match the project name.
- Add tagged, cross-platform release builds for Linux x86-64 and ARM64, Windows x86-64, macOS Intel, and Apple Silicon.
- Improve community contribution templates and project documentation.

## 0.1.0 - Initial public release

- Interactive ratatui execution-tree viewer.
- Geth-style `callTracer` JSON parser.
- Foundry `forge test -vvvv` trace parsing.
- Foundry `VM::...` cheatcode preservation.
- Foundry suite pass/fail outcome preservation.
- Foundry `-vvvvv` storage-change parsing.
- Built-in selector recognition for common ERC-20, proxy and ownership methods.
- Optional ABI selector map.
- Frame-local storage diff display.
- Revert and error visibility.
- Revert-only and storage-write filters.
- Panic-safe terminal restoration.
- Deterministic Cargo dependency lockfile.
- Multi-platform CI and dependency auditing.
