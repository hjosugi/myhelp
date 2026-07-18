# Architecture

## Shape

```text
                  plain Markdown vault
                           │
                    myhelp-core (Rust)
                    ╱               ╲
          myhelp CLI                  Tauri commands
   local shell / SSH                   │
                                React desktop editor
```

`myhelp-core` is the only layer allowed to define page paths and storage
semantics. The CLI and desktop adapter call it directly. React accesses files
only through typed Tauri commands.

## Components

### `myhelp-core`

- Resolves the platform data directory.
- Honors `MYHELP_PAGES_DIR`.
- Validates topics and blocks path traversal.
- Scans without following symlinks.
- Reads and writes UTF-8 Markdown.
- Lists and searches pages.

The current write implementation is intentionally simple. Atomic writes,
backups, conflict detection, and file watching are tracked as follow-up work.

### `myhelp-cli`

- Uses `clap` for portable argument parsing.
- Prints raw Markdown in the scaffold; terminal rendering is a separate issue.
- Opens `$VISUAL` or `$EDITOR` for editing.
- Does not execute page commands.

### `myhelp-desktop`

- Tauri 2 hosts a React/TypeScript UI in the operating system webview.
- Rust commands are thin adapters over `myhelp-core`.
- The initial UI provides page search, creation, editing, saving, and Markdown
  preview.

### Cargo workspace boundary

The root Cargo workspace contains `myhelp-core` and `myhelp-cli`, with
`Cargo.lock` recording only their portable dependency graph. `src-tauri` is a
separate Cargo workspace with `src-tauri/Cargo.lock` for desktop-only
dependencies. This keeps CLI packaging from fetching Tauri, Wry, WebKitGTK, or
platform webview crates.

Both interfaces still compile the same `crates/myhelp-core` source through a
path dependency. The split is a build and dependency-lock boundary, not a fork
of storage behavior. Dependency updates must refresh both lockfiles explicitly:

```bash
cargo update --workspace
cargo update --manifest-path src-tauri/Cargo.toml --workspace
```

### Storage

`MYHELP_PAGES_DIR` has highest priority. Otherwise the `directories` Rust crate
selects the per-user application data directory for Linux, macOS, or Windows.

The vault is portable. A user may place it in Git, Syncthing, Dropbox, or
another sync folder without MyHelp knowing about that provider.

## Security model

- Topics cannot be absolute or contain `..`.
- Vault scans do not follow symlinks.
- Markdown is rendered without raw HTML in the React scaffold.
- Saved commands are text only and are never executed by the MVP.
- Tauri capabilities should remain minimal.

Any command-execution feature requires a separate design covering confirmation,
shell quoting, untrusted imports, environment access, and auditability.

## Cross-platform contract

Core and CLI tests run on Linux, macOS, and Windows. Desktop packaging will use
native runners because Tauri applications are built on the target platform.
Platform-specific behavior must stay behind core path resolution or narrow
adapter modules.
