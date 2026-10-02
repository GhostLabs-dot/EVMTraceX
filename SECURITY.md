# Security Policy

## Reporting a Vulnerability

The EVM Execution Trace Explorer is intended for local EVM trace analysis, debugging, auditing, and security research.

If you discover a security vulnerability in the application, parser, dependency configuration, build system, or release artifacts, please report it privately.

Please do not disclose security vulnerabilities through public GitHub issues, pull requests, or other public channels before the issue has been reviewed.

The preferred reporting channel is GitHub's private vulnerability reporting / Security Advisory functionality for this repository.

When reporting a vulnerability, please include enough information to reproduce the issue, such as:

* affected version or commit;
* operating system and Rust toolchain version when relevant;
* the affected input format or trace type;
* a minimal reproduction case;
* expected behavior;
* actual behavior;
* any security or integrity impact you identified.

Do not include private keys, credentials, API secrets, or other sensitive information unless it is strictly necessary to demonstrate the issue.

## Scope

Security reports may include issues involving:

* trace parsing and normalization;
* malformed or adversarial trace input;
* ABI and selector handling;
* RPC request handling;
* terminal or application state handling;
* dependency vulnerabilities;
* CI or release configuration;
* packaged binaries and release artifacts.

The project does not itself execute transactions or mutate blockchain state. RPC mode only sends a transaction hash to the explicitly supplied RPC endpoint in order to request a completed transaction trace.

## Disclosure

Ghost Labs will review valid reports, investigate their impact, and coordinate remediation and disclosure with the reporter where appropriate.

Please allow reasonable time for investigation and remediation before public disclosure.

## Supported Versions

Security fixes are generally targeted at the latest maintained release and the current development branch.

Users are encouraged to keep the tool and its dependencies up to date.
