# Changelog

This changelog tracks published versions. Git tags that were created but never published as releases are called out explicitly.

## [0.2.1] — 2026-10-16

**Published release:** https://github.com/GhostLabs-dot/EVMTraceX/releases/tag/v0.2.1

- Rename the Cargo package and executable from `evmtrace-tui` to `evmtracex`.
- Publish downloadable release binaries for Linux x86-64, Linux ARM64, Windows x86-64, macOS Intel, and macOS Apple Silicon.
- Improve community contribution files and project documentation.
- Update the Rustls dependency to patched version `0.23.46`.

## [0.2.0] — legacy tag, not a published release

- The existing Git tag `v0.2.0` points to the earlier source that still declares package version `0.1.0` and executable name `evmtrace-tui`.
- No GitHub Release or binary assets were published for `v0.2.0`.
- The tag is preserved for history rather than moved; the renamed application and its first downloadable binaries were published as `v0.2.1`.

## [0.1.0] — 2026-10-04

**Initial public release:** https://github.com/GhostLabs-dot/EVMTraceX/releases/tag/v0.1.0

- Interactive ratatui execution-tree viewer.
- Geth-style `callTracer` JSON parser.
- Foundry `forge test -vvvv` trace parsing.
- Foundry `VM::...` cheatcode preservation.
- Foundry suite pass/fail outcome preservation.
- Foundry `-vvvvv` storage-change parsing.
- Built-in selector recognition for common ERC-20, proxy, and ownership methods.
- Optional ABI selector map.
- Frame-local storage diff display.
- Revert and error visibility.
- Revert-only and storage-write filters.
- Panic-safe terminal restoration.
- Deterministic Cargo dependency lockfile.
- Multi-platform CI and dependency auditing.
