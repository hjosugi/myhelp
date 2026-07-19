# MyHelp

<!-- markdownlint-disable MD033 -->

<p align="center">
  <img
    src="assets/brand/myhelp-mark.svg"
    width="112"
    height="112"
    alt="MyHelp mark: a help page with a folded corner and an M"
  />
</p>

<!-- markdownlint-enable MD033 -->

MyHelp is a local-first, cross-platform home for the commands and procedures you
do not use often enough to memorize.

It keeps help pages as plain Markdown and exposes the same vault through:

- a fast Rust CLI for terminals and remote sessions;
- a Tauri desktop editor with search and live preview;
- a reusable Rust core shared by both interfaces;
- import/export adapters for established cheatsheet formats, starting with
  tldr/tealdeer.

> [!IMPORTANT]
> This repository is an early public scaffold. The storage core, CLI commands,
> and desktop editor are intentionally small so subsequent work can be driven by
> focused GitHub issues.

## Why another tool?

MyHelp does not aim to replace the mature tools around it:

- [tldr pages](https://github.com/tldr-pages/tldr) defines the concise
  example-oriented Markdown format.
- [tealdeer](https://tealdeer-rs.github.io/tealdeer/) is an excellent terminal
  renderer and already supports custom pages.
- [navi](https://github.com/denisidoro/navi),
  [cheat](https://github.com/cheat/cheat), and
  [pet](https://github.com/knqyf263/pet) are mature interactive command and
  snippet managers.
- [massCode](https://masscode.io/) is a broad desktop workspace with a strong
  Markdown vault.

The narrower gap MyHelp explores is: one portable, local folder with a small
CLI and a focused GUI editor, while remaining interoperable with those formats
instead of inventing another closed database. See
[the research notes](docs/research.md).

## Current scaffold

```text
myhelp
├── crates/myhelp-core    storage, validation, import/export, page rules
├── crates/myhelp-cli     terminal UX, tldr adapters, completions
├── src                   React editor and Markdown preview
├── src-tauri             Tauri commands that call myhelp-core
└── docs                  architecture, format, research, roadmap
```

No page content is uploaded, and the MVP does not execute saved commands.

## Development

The repository includes a Nix Flake for Linux development:

```bash
direnv allow
pnpm install

cargo test -p myhelp-core -p myhelp-cli
cargo run -p myhelp-cli -- list
pnpm test
pnpm build
pnpm tauri dev
```

Without direnv, run `nix develop` first. Native setup for macOS and Windows is
documented by the
[official Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/).

The CLI is also exposed as a Nix app on Linux and macOS:

```bash
nix run github:hjosugi/myhelp -- list
nix build github:hjosugi/myhelp#myhelp-cli
```

This packages only the portable CLI. Desktop packaging remains native
platform work. CI builds an unsigned native package on Linux, macOS, and
Windows and retains each package for seven days as an inspection artifact.
These smoke artifacts are not advertised distribution channels; see the
[packaging checks](docs/packaging.md).

## CLI preview

```bash
myhelp list
myhelp new python/new-project --title "New Python project"
myhelp edit python/new-project
myhelp show python/new-project
myhelp show python/new-project --raw
myhelp search pytest
myhelp pick
myhelp list --json
myhelp tldr validate ./git.md
myhelp tldr import ./git.page.md --topic work/git
myhelp tldr export ./tealdeer-pages --json
myhelp adapter inspect navi ./git.cheat --json
myhelp completions fish > ~/.config/fish/completions/myhelp.fish
myhelp path
```

Set `MYHELP_PAGES_DIR` or pass `--pages-dir` to use an existing Markdown vault.
On a terminal, `show` renders the tldr-style Markdown with terminal-aware color
and opens an internal pager only when the rendered page is taller than the
screen. Piped output remains the original Markdown, and `NO_COLOR` or
`--color never` disables styling. `pick` is an explicit fuzzy selector; it only
displays or prints a topic and never executes a saved command.

`VISUAL` and `EDITOR` may include arguments such as `code --wait`. MyHelp parses
the configured command for the current platform and starts it directly without
invoking a shell. Completion setup, JSON output, pager behavior, shell
integration, and stable exit codes are documented in the
[CLI contract](docs/cli.md).

The tldr adapter preserves imported page bytes, reports line-oriented
diagnostics, and exports nested topics through a deterministic,
case-insensitive collision-safe mapping without overwriting existing files.
Flat vaults can also be used directly as a tealdeer custom-page directory.
See the [tldr and tealdeer interoperability contract](docs/tldr.md).

The navi prototype generates a typed, explicitly lossy page preview without
creating a vault or executing snippets and command-backed variable sources.
cheat and pet remain documented read-only-index targets until their unstructured
or multi-snippet metadata can be mapped honestly. See the
[foreign-format adapter contract](docs/adapters.md).

## Data safety

MyHelp stages every page save in the page's directory, flushes and syncs the
complete draft, then atomically replaces the old file. Reads carry a revision
made from the file modification time and SHA-256 content hash, so the CLI and
desktop editor refuse to overwrite a page changed by another editor, Git, or a
sync tool.

`myhelp edit` edits a temporary working copy rather than the live page. On a
conflict, the disk page remains untouched and MyHelp stores the draft as a
readable adjacent `*.page.conflict-<sha256>.md` file. The desktop app
automatically reloads clean pages and presents reconciliation choices when a
page with unsaved edits changes or is deleted externally.

The desktop editor also confirms every context-changing action while a draft is
dirty, including navigation, creation, rename, deletion, vault switching, and
window close. Rename never replaces another topic. Delete moves the page to a
readable adjacent recovery file and offers Undo instead of unlinking it
permanently.

MyHelp does not follow symlinks or Windows reparse points while accessing a
vault. Pages are bounded to 1 MiB before MyHelp allocates or writes them. See
the [cross-platform storage contract](docs/architecture.md#data-integrity-and-external-edits).

The desktop app uses an explicit production CSP, inert Markdown links and
images, a native navigation guard, and a main-window-only Tauri capability.
See the [desktop threat model](docs/security.md) and
[security reporting policy](SECURITY.md). Keyboard shortcuts, focus behavior,
screen-reader announcements, contrast checks, and the manual accessibility
matrix are documented in the
[desktop accessibility contract](docs/accessibility.md).

## Language workflow starter pack

The repository includes concise new-project and daily-command pages for the
language toolchains used to shape the initial product:

```bash
cargo run -p myhelp-cli -- \
  --pages-dir examples/language-workflows list
cargo run -p myhelp-cli -- \
  --pages-dir examples/language-workflows show python-new-project
```

The pack currently covers Python, Go, Rust, Node.js/TypeScript, Java, Lua,
Elixir, Gleam, Haskell, Zig, Ruby, Common Lisp, and C/C++. Every page links to
the relevant official documentation. The examples are portable defaults; copy
and customize them in your own vault rather than coupling the public project to
one machine's dotfiles.

## File format

MyHelp starts with the tldr custom-page convention:

```markdown
# New Python project

> Start a reproducible Python project with Nix and uv.

- Create and enter the development environment:

`nix flake lock && direnv allow`
```

Flat `<topic>.page.md` files can be consumed directly by tealdeer when its custom
page directory points at the vault. Nested topics are a MyHelp organization
extension and use an explicit export mapping. See
[the format contract](docs/format.md) and
[the tldr adapter guide](docs/tldr.md). Foreign navi, cheat, and pet boundaries
are documented in [the foreign-format adapter guide](docs/adapters.md).

## Official references

- [Tauri 2](https://v2.tauri.app/start/)
- [Tauri architecture](https://v2.tauri.app/concept/architecture/)
- [tldr page syntax](https://github.com/tldr-pages/tldr/blob/main/contributing-guides/style-guide.md)
- [tealdeer custom pages](https://tealdeer-rs.github.io/tealdeer/usage_custom_pages.html)
- [Rust CLI with clap](https://docs.rs/clap/latest/clap/)

## Contributing

Read [AGENTS.md](AGENTS.md), [CONTRIBUTING.md](CONTRIBUTING.md), and the open
GitHub issues before implementing a feature. Each issue is designed to be
workable by a contributor or a coding agent without requiring private dotfiles.

## License

[MIT](LICENSE). The original MyHelp mark and its generated application icons
are also MIT-licensed; see the editable
[brand source and color tokens](assets/brand/README.md).
