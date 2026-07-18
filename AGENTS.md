# AGENTS.md

## Mission

Build MyHelp as a local-first, cross-platform CLI and desktop editor for small
personal help pages. Preserve interoperability with established plaintext
formats and avoid duplicating mature command-execution or cloud-sync products.

## Read first

Before changing code, read:

1. `README.md`
2. `docs/research.md`
3. `docs/architecture.md`
4. `docs/format.md`
5. the GitHub issue you are implementing

If an issue conflicts with these documents, explain the conflict on the issue
before changing the architecture.

## Repository map

- `crates/myhelp-core`: platform-independent storage and page rules.
- `crates/myhelp-cli`: terminal UX; it must depend on core rather than duplicate it.
- `src-tauri`: a thin Tauri adapter over core.
- `src`: React UI; filesystem access must go through typed Tauri commands.
- `docs`: decisions, research, and interoperability contracts.

## Development environment

On Linux:

```bash
direnv allow
pnpm install
```

Portable checks:

```bash
cargo fmt --all --check
cargo test -p myhelp-core -p myhelp-cli
cargo clippy -p myhelp-core -p myhelp-cli --all-targets -- -D warnings
pnpm build
```

Run the app:

```bash
cargo run -p myhelp-cli -- list
pnpm tauri dev
```

## Guardrails

- Keep user data as readable files. Do not introduce a required database.
- Do not add command execution to the MVP. Displaying and copying commands is
  allowed; execution requires a separate threat model and issue.
- Do not invent a new page syntax when tldr, navi, or cheat compatibility can
  solve the use case through an adapter.
- Never follow symlinks while scanning a vault.
- Reject absolute paths and parent-directory traversal in topics.
- Keep the core crate independent of Tauri and React.
- Add tests for storage or format changes before wiring them into the GUI.
- Treat imported third-party cheatsheet content and its license separately from
  MyHelp's MIT-licensed source code.
- Do not depend on the maintainer's dotfiles or machine-specific paths.

## GitHub workflow

- Take one issue at a time.
- Branch from `main` as `agent/issue-<number>-<slug>`.
- Keep commits focused and include the issue number in the PR body.
- Open a draft PR while work is in progress.
- Update documentation when behavior or the format contract changes.

## Definition of done

- Relevant tests pass on the current platform.
- `cargo fmt --all --check` and `pnpm build` pass.
- New public behavior is documented.
- Cross-platform path handling is considered explicitly.
- No unrelated generated or user files are committed.
