# Contributing

Thanks for helping improve `evmtrace-tui`.

## Development

```bash
cargo fmt
cargo check --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
```

Keep parser changes independent from UI changes where practical. New trace formats should map into `TraceNode` instead of adding format-specific rendering logic.

## Pull requests

Include a focused description and a small trace fixture when the change affects parsing or rendering behavior.
