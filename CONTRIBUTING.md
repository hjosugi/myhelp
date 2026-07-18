# Contributing

Thank you for helping build MyHelp.

Start by choosing an open issue. The first milestones deliberately separate the
storage core, CLI, GUI, interoperability, and release work so changes remain
reviewable.

## Setup

Linux contributors can use the repository Flake:

```bash
direnv allow
pnpm install
```

For native macOS or Windows setup, follow the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## Before opening a pull request

```bash
cargo fmt --all --check
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo test --workspace --locked
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
pnpm build
```

Include:

- the problem and intended user outcome;
- the issue being addressed;
- tests or manual validation performed;
- screenshots for visible desktop changes;
- any format or cross-platform compatibility impact.

MyHelp source is MIT-licensed. Do not copy third-party cheatsheet content into
the repository unless its license and attribution requirements are documented.
