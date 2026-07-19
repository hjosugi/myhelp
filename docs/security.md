# Desktop threat model

Status: accepted for the pre-1.0 desktop preview on 2026-07-19.

This document describes the security boundary of the local MyHelp desktop app.
It complements the storage rules in
[`docs/architecture.md`](architecture.md#data-integrity-and-external-edits).

## Protected assets

- Help-page content and conflict drafts in the selected vault.
- Other files on the local machine, which MyHelp must not read or modify.
- The native command boundary exposed by Tauri IPC.
- The integrity of the bundled frontend and release process.

MyHelp assumes that the operating-system account and the application binary are
trusted. Vault contents, filenames, Markdown, external editor changes, imported
content, and filesystem watcher events are untrusted.

## Security boundaries

### Filesystem and resource limits

`myhelp-core` owns every filesystem-relevant rule. Topics are relative,
portable paths with no empty, current-directory, parent-directory, backslash,
absolute, or `.page.md` suffix forms. A topic is at most 240 UTF-8 bytes and a
search query is at most 1,024 UTF-8 bytes.

Vault roots cannot be symlinks or Windows reparse points. Scan, read, create,
save, and conflict-copy paths reject links and non-regular files below the
vault boundary. The platform may resolve an operating-system link above the
configured vault boundary, such as macOS `/var` to `/private/var`.

A page is at most 1 MiB. Core checks caller-provided content before creating
files, checks external file metadata before allocation, and limits the read
itself to one byte beyond the maximum before rejecting it. This keeps unusually
large or concurrently growing files from causing unbounded page allocations.

Atomic replacement and revision checks protect complete files and preserve
both versions of a concurrent edit. They do not provide encryption, access
control, malware scanning, or a substitute for operating-system backups.

### Markdown preview and URLs

Markdown is data, never application code:

- `react-markdown` runs with raw HTML disabled.
- Link destinations are displayed as inert text without an `href`.
- Images are replaced with inert descriptions without a `src`, preventing
  remote tracking requests and local-file reads.
- The native webview navigation guard allows only bundled application assets.
  Development builds additionally allow the exact Vite origin
  `http://localhost:1420`.
- MyHelp does not open external links yet. A future opener requires an explicit
  allowlist, user gesture, safe operating-system API, and separate review.

The production Content Security Policy allows only bundled scripts, styles, and
fonts, local or data images, and Tauri IPC. It denies objects, frames, forms,
media, and every other network connection. Tauri's build-time nonce and hash
injection remains enabled. The separate development policy adds only the Vite
websocket and development requirements; those sources do not ship in release
builds. The custom-protocol `Object.prototype` is frozen.

### IPC and capabilities

Only the local window labelled `main` receives the `main-editor` capability.
It grants event `listen` and `unlisten`, plus these application commands:

- `list_pages`
- `read_page`
- `save_page`
- `preserve_draft`
- `restore_page`
- `create_page`
- `search_pages`
- `get_vault_path`
- `rename_page`
- `delete_page`
- `restore_deleted_page`
- `choose_vault`
- `close_window`

The command list is declared in the Tauri application manifest, so each command
requires its generated allow permission. The dialog plugin is initialized only
for the native `choose_vault` implementation; none of its frontend permissions
are granted. No shell, process, filesystem, dialog, HTTP, clipboard, updater,
tray, menu, path, or window-management plugin permission is enabled. Remote
origins are not included in the capability.

The Rust adapter passes topics and content to core rather than constructing
page paths. The dedicated vault chooser returns only the directory the user
selected, validates it as a vault, and replaces the native watcher; it does not
grant the WebView general filesystem access. Structured errors do not include
page content. The vault path, conflict path, and deletion recovery state are
shown to the local user; they are not transmitted.

### Saved commands

Command examples are rendered, edited, and copied as text. The MVP has no
command-execution backend or shell plugin. Execution would add shell parsing,
environment, imported-content, confirmation, privilege, and audit risks and
requires a separate threat model before implementation.

## Threat review

<!-- markdownlint-disable MD013 MD060 -->

| Threat | Control | Residual risk |
|---|---|---|
| Markdown XSS or HTML injection | Raw HTML is skipped; hostile-preview tests assert no script, iframe, event attribute, navigable link, or fetchable image | Bugs in React, react-markdown, or the system webview |
| External URL, `javascript:`, `data:`, or `file:` navigation | Links are inert; Rust navigation guard rejects non-app origins; CSP blocks remote resources | A future external-link feature must be reviewed separately |
| Path traversal or link escape | Core validation, no-follow opens, link/reparse checks, and cross-platform tests | Filesystem races are reduced but ordinary filesystems do not offer a portable directory-handle API |
| Blind overwrite by editor, Git, or sync tool | Modification-time plus SHA-256 revision checks, atomic replace, and conflict copies | A narrow final compare/rename race remains documented |
| Memory pressure from hostile pages or IPC input | 1 MiB page, 240-byte topic, and 1,024-byte query limits, including bounded reads | Many valid pages can still consume time during a full search |
| Frontend compromise reaching native APIs | Explicit local capability and per-command permissions; no general filesystem or shell plugin | Allowed page commands can still modify the user's configured vault |
| Development policy shipping to production | Separate `devCsp`; production policy contains no localhost, websocket, eval, or inline-style source | Release configuration still requires CI and review |
| CI action tag retargeting | Third-party GitHub Actions are pinned to full commit SHAs | Maintainers must deliberately update pins for security releases |

<!-- markdownlint-enable MD013 MD060 -->

## Known upstream dependency risk

The Linux Tauri 2.11.5 dependency graph currently includes `glib 0.18.5`
through the archived GTK3 bindings. GitHub advisory
[`GHSA-wrw7-89jp-8q8g`](https://github.com/advisories/GHSA-wrw7-89jp-8q8g)
reports undefined behavior in `glib::VariantStrIter`; the published fix is
`glib 0.20.0`, which cannot satisfy the GTK3 dependency constraint.

This is a constrained, temporary risk acceptance, not a claim that the package
is patched:

- MyHelp does not call `VariantStrIter` or `Variant::array_iter_str`.
- A source search of the resolved Tauri, Wry, Tao, GTK, and WebKitGTK crates
  found no call site outside `glib` itself.
- Tauri maintainers track the same unresolved advisory in
  [`tauri-apps/wry#1769`](https://github.com/tauri-apps/wry/issues/1769) and
  plan the maintained GTK4 path for Tauri v3.

Recheck the dependency tree on every Tauri update and before a stable MyHelp
release. Remove this acceptance as soon as an upstream-compatible patched stack
exists; do not vendor an unaudited GTK fork merely to silence the scanner.

## Verification

Security-relevant changes must keep these checks green:

```bash
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
pnpm test
pnpm build
pnpm tauri build --no-bundle
```

Core and desktop Rust checks run on Linux, macOS, and Windows in CI. Hostile
Markdown tests and the production frontend build run on Node.js 24.

## References

- [Tauri Content Security Policy](https://v2.tauri.app/security/csp/)
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri core permissions](https://v2.tauri.app/reference/acl/core-permissions/)
- [Tauri webview navigation hook](https://docs.rs/tauri/latest/tauri/plugin/struct.Builder.html#method.on_navigation)
- [glib advisory](https://github.com/advisories/GHSA-wrw7-89jp-8q8g)
