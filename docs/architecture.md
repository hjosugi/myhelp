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
- Rejects symlinks and Windows reparse points in scan, read, and write paths.
- Reads and writes UTF-8 Markdown.
- Atomically replaces pages only when their last-read revision still matches.
- Renames pages without replacing an occupied topic.
- Moves deleted pages to readable recovery Markdown and restores them on demand.
- Lists and searches pages.

The accepted [page metadata ADR](adr/0001-page-metadata-sidecars.md) assigns
optional sidecar path rules, parsing, validation, and diagnostics to core. That
contract is not implemented yet; CLI and Tauri adapters must not grow their own
metadata parser while implementation is pending.

### `myhelp-cli`

- Uses `clap` for portable argument parsing.
- Prints raw Markdown in the scaffold; terminal rendering is a separate issue.
- Opens a temporary working copy in `$VISUAL` or `$EDITOR`, then asks core to
  save against the last-read revision.
- Preserves a readable conflict copy when a disk edit wins the revision check.
- Does not execute page commands.

### `myhelp-desktop`

- Tauri 2 hosts a React/TypeScript UI in the operating system webview.
- Rust commands are thin adapters over `myhelp-core`.
- The UI provides page search, creation, editing, saving, rename, recoverable
  deletion, Markdown preview, and a native vault chooser.
- Dirty navigation, creation, rename, deletion, vault switching, and native
  window close all pass through one explicit save/discard/cancel decision.
- The vault chooser is implemented as a dedicated Rust command. The dialog
  plugin's general frontend commands receive no capability, and the selected
  path is validated by core before the watcher and command state switch.
- A recursive native watcher emits small Tauri events. The React editor
  debounces them, reloads clean pages, and enters a recoverable conflict state
  for dirty or externally deleted pages.
- If the native watcher cannot start, the adapter falls back to a two-second,
  content-aware polling watcher. Window focus and visibility changes also
  trigger revision checks.

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

Conflict copies are adjacent plain Markdown files named
`<topic>.page.conflict-<content-sha256>.md`. They deliberately do not end in
`.page.md`, so normal listing and tldr/tealdeer consumers ignore them. Repeating
the same conflicted draft reuses the same copy.

Recoverable deletions are adjacent plain Markdown files named
`<topic>.page.deleted-<content-sha256>[-n].md`. They also stay out of normal
listing. Undo moves the recovery file back only when the original topic remains
unoccupied. A MyHelp rename, deletion, or restore carries an existing
`<topic>.page.meta.yaml` sidecar without parsing it.

Metadata, when present, is an adjacent readable YAML sidecar. Markdown-only
vaults remain valid, and metadata failures do not hide readable pages. Core
will expose a typed metadata state so the CLI and Tauri adapter can report the
same missing, invalid, conflict, or unsupported-version result.

## Security model

- The reviewed desktop threat model lives in
  [`docs/security.md`](security.md).
- Topics cannot be absolute or contain `..`.
- Vault scans do not follow symlinks.
- Pages are limited to 1 MiB; topics and search queries also have explicit
  bounds.
- Markdown is rendered without raw HTML, active links, or external images.
- Saved commands are text only and are never executed by the MVP.
- Production and development use separate CSPs.
- The main window can listen for events and invoke only the thirteen typed
  MyHelp commands declared in the Tauri application manifest.

Any command-execution feature requires a separate design covering confirmation,
shell quoting, untrusted imports, environment access, and auditability.

## Data integrity and external edits

Every create or save follows this order:

1. validate the portable topic and reject a symlinked vault root plus every
   detected symlink or Windows reparse point below it through the target;
2. for saves, compare the current modification time and SHA-256 content hash
   with the revision returned by the last read;
3. write the complete UTF-8 content to a randomly named temporary file in the
   destination directory, flush it, and sync it to the filesystem;
4. recheck the target revision after staging, then atomically replace the
   destination;
5. read the committed page again and return its new revision.

Rename, recoverable delete, and restore compare the last-read revision, reject
symlink/reparse paths, and use the platform's atomic no-replace rename primitive
(`renameat2(RENAME_NOREPLACE)`, `renamex_np(RENAME_EXCL)`, or `MoveFileW`).
An occupied destination therefore fails without replacing it. Page and metadata
sidecar moves cannot be a single portable transaction; if the sidecar move
fails, core attempts to move the already-safe page back to its source name and
reports both errors if rollback also fails. A missing or duplicated sidecar
never hides readable Markdown, matching the metadata ADR's degradation rule.

If either revision check fails, core returns a typed conflict and does not
commit the temporary file. There is an unavoidable final compare/replace window
on ordinary filesystems because they do not expose a portable conditional
rename primitive; staging the full content and rechecking immediately before
the rename minimizes that window. A committed path always exposes either the
old complete file or the new complete file, never an in-place partial write.

<!-- markdownlint-disable MD013 MD060 -->

| Platform | Replacement and watcher behavior |
|---|---|
| Linux | Same-directory temporary file, `fsync`, and `renameat`; native inotify watcher with polling fallback. |
| macOS | Same-directory temporary file, `fsync`, and rename; native FSEvents watcher with polling fallback. |
| Windows | Same-directory temporary file is flushed and replaced through Rust's Windows rename path (`MoveFileExW` or `SetFileInformationByHandle`). Windows 10 1607+ filesystems supporting `FileRenameInfoEx` have Unix-like file replacement behavior. If permissions, an open handle, antivirus, or the filesystem rejects replacement, save returns an error and the old page remains. Native `ReadDirectoryChangesW` watching falls back to polling when unavailable. |

<!-- markdownlint-enable MD013 MD060 -->

Native watcher events are hints rather than save authorization: every reaction
re-reads and compares the core revision. This handles editors that truncate in
place as well as editors and sync tools that replace files.

Rolling backups are not part of the MVP. Automatic generations would introduce
retention, privacy, and sync-noise policy without solving concurrent edits.
Atomic replacement protects against partial writes; deterministic readable
conflict copies protect user drafts when revisions diverge.

## Cross-platform contract

Core, CLI, atomic replacement, revision conflict, and path-attack tests run on
Linux, macOS, and Windows. Desktop Rust compilation and tests also use native
runners so every watcher backend remains build-checked. Platform-specific
behavior stays behind core path resolution or narrow adapter modules.

Each native desktop job then builds one representative unsigned package: a
Debian package on Linux, a disk image on macOS, and an NSIS installer on
Windows. Package creation is a smoke test rather than a release channel. CI
uploads the result for seven days with no release-signing keys or repository
write permission; see [`docs/packaging.md`](packaging.md).

The atomic staging implementation is provided by
[`atomic-write-file`](https://docs.rs/atomic-write-file/0.3.0/atomic_write_file/);
the desktop watcher uses [`notify`](https://docs.rs/notify/8.2.0/notify/).
