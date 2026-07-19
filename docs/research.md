# Prior-art research

Snapshot date: 2026-07-19.

MyHelp should be a narrow interoperability layer and editor, not a rewrite of
the mature tools below.

<!-- markdownlint-disable MD013 MD060 -->

| Project | What it already does well | What MyHelp should reuse or support |
|---|---|---|
| [tldr-pages/tldr](https://github.com/tldr-pages/tldr) | Widely adopted, concise example-oriented Markdown and a published client specification | Use its page structure as the first storage/export contract |
| [tealdeer](https://github.com/tealdeer-rs/tealdeer) | Fast Rust terminal rendering, custom pages and patches, OS-specific custom page locations | Support `<topic>.page.md`; allow a MyHelp vault to be used as tealdeer's custom-page directory |
| [navi](https://github.com/denisidoro/navi) | Fuzzy terminal search, argument placeholders, shell widgets, Git-hosted cheatsheet repositories, tldr/cheat.sh imports | Learn from its search and repository UX; add an adapter instead of copying its execution engine |
| [cheat](https://github.com/cheat/cheat) | Editable layered cheatpaths, tags, syntax highlighting, nested sheets, search | Learn from cheatpath layering and tag UX; keep compatibility behind an adapter |
| [pet](https://github.com/knqyf263/pet) | Focused CLI snippet capture and remote snippet synchronization | Treat Gist/Git synchronization as optional future adapters, not required storage |
| [massCode](https://github.com/massCodeIO/massCode) | Polished desktop editor, local Markdown vault, tags, file watching, broad developer workspace | Learn from its vault/editor UX; keep MyHelp smaller, terminal-first, and tldr-oriented |

<!-- markdownlint-enable MD013 MD060 -->

At the time of this snapshot, these projects are active and mature: tldr has
over 63k GitHub stars, navi over 17k, cheat over 13k, massCode over 6k,
tealdeer over 6k, and pet over 5k. Popularity is not a product requirement, but
it is strong evidence that MyHelp should integrate rather than replace.

## Product gap

The initial hypothesis is that developers want:

1. a tiny CLI available in local shells and SSH sessions;
2. a focused cross-platform GUI for authoring the same files;
3. plain files suitable for Git, Syncthing, Dropbox, or any other sync tool;
4. a documented path into and out of tldr/tealdeer, navi, and cheat;
5. no account, server, or opaque database.

massCode covers a much broader GUI workspace. navi, cheat, pet, and tealdeer
cover terminal workflows very well. MyHelp is only justified if it stays focused
on the shared CLI/GUI authoring gap and interoperability.

## Metadata comparison

<!-- markdownlint-disable MD013 MD060 -->

| Project | Metadata/storage model | MyHelp implication |
|---|---|---|
| tldr | CommonMark pages; platform and locale are represented by directories | Keep compatible Markdown untouched and map locale during export |
| tealdeer | Exact `<name>.page.md` custom pages and `<name>.patch.md` patches | Preserve direct custom-page use; do not require MyHelp syntax in the body |
| navi | `.cheat`/`.cheat.md` lines encode tags, descriptions, variables, and executable snippets | Treat its metadata and execution-oriented syntax as an adapter format |
| cheat | Extensionless text with optional YAML front matter for tags and syntax | Map supported fields on export, but do not adopt front matter for tldr pages |
| pet | TOML records hold description, command, tags, and output | Import/export only the honest intersection and never execute imported commands |
| massCode | Markdown vault with front matter metadata and separate UI state | Reuse the local-file and live-update lessons while keeping MyHelp pages tldr-compatible |

<!-- markdownlint-enable MD013 MD060 -->

The resulting decision is an optional adjacent sidecar, documented in
[ADR 0001](adr/0001-page-metadata-sidecars.md). This gives tags, stable IDs,
locale, and licensing a typed home without changing the page body or requiring
a database.

## Foreign-adapter prototype findings

The July 2026 prototype compared pinned upstream parser and storage models, not
only README examples:

<!-- markdownlint-disable MD013 MD060 -->

| Tool | Honest intersection | Material losses or boundaries | Compatibility decision |
|---|---|---|---|
| navi | `#` descriptions, single-line snippets, and `<name>` placeholders map to tldr examples and `{{name}}` | `%` tags are report-only; `$` command-backed suggestions and `@` inheritance are not evaluated; multiline snippets and multiple contexts need an explicit split policy | Implemented lossy import preview with typed dry-run diagnostics |
| cheat | Filename/title and merged sheet plus cheatpath tags can become candidate topic and metadata; a body that already validates as tldr could pass through | General bodies are free-form; `syntax`, cheatpath identity, and read-only policy do not fit the page body; automatic structuring would be invented | Read-only index first |
| pet | A description plus single-line command can become one example; simple `<name>` placeholders can map | Multiline commands, `<name=default>`, output, tags, multi-snippet grouping, and source filenames need policy; sync backends, tokens, and timestamps are operational state | Read-only index first, then explicit per-snippet dry-run previews |

<!-- markdownlint-enable MD013 MD060 -->

No adapter currently claims export or round-trip compatibility. In particular,
navi and pet consume selected text as executable commands, while MyHelp's page
contract is display-only. Their program licenses are Apache-2.0 for navi and MIT
for cheat and pet, but imported sheet content keeps its own license. Current
test fixtures are original MIT-licensed MyHelp data.

The accepted field-by-field decision, exact upstream revisions, and security
boundary are in
[ADR 0002](adr/0002-foreign-format-adapter-levels.md). The user-facing dry-run
contract is in [the foreign adapter guide](adapters.md).

## Focused editor benchmark

A small editor still needs the reliability baseline users learn from mature
developer tools:

<!-- markdownlint-disable MD013 MD060 -->

| Reference behavior | Competitive bar for MyHelp | Current decision |
|---|---|---|
| [massCode command palette](https://masscode.io/documentation/command-palette.html) opens content and creates items from the keyboard | Search, create, save, and view switching must not require a mouse | Direct cross-platform shortcuts cover the narrow primary workflow; a larger command palette waits until there are enough commands to justify one |
| [massCode Markdown vault](https://masscode.io/documentation/storage.html) supports selectable plain-file storage and live external updates | Vault choice cannot require environment variables, and watcher refresh cannot discard edits | A native chooser switches the core-owned vault; revision checks and readable conflict copies preserve concurrent drafts |
| [massCode editor preferences](https://masscode.io/documentation/notes/) keep typography roles consistent and configurable | The initial UI must not accumulate unrelated font sizes, line heights, or colors | Shared semantic CSS properties define all component type sizes and light/dark palettes; tests reject one-off font-size declarations |
| [VS Code Hot Exit](https://code.visualstudio.com/docs/editing/codebasics#_hot-exit) preserves unsaved work across exit | Navigation and native close cannot silently lose a draft | One save/discard/cancel state handles every context change; delete is an adjacent recovery file with Undo |
| [VS Code accessibility](https://code.visualstudio.com/docs/configure/accessibility/accessibility) treats keyboard, focus, high contrast, zoom, and screen readers as product behavior | Accessibility needs an explicit contract and release matrix, not only semantic markup | MyHelp documents shortcuts, modal focus, live status, contrast-token tests, reduced motion, forced colors, axe checks, and native screen-reader checks |

<!-- markdownlint-enable MD013 MD060 -->

MyHelp does not need massCode's broad workspace or VS Code's configurable
workbench to meet this bar. It does need equal care for the smaller set of
actions it exposes: no hidden data loss, no mouse-only path, no opaque storage,
and no visual value that drifts outside the shared role system.

## Focused terminal benchmark

<!-- markdownlint-disable MD013 MD060 -->

| Reference behavior | Competitive bar for MyHelp | Current decision |
|---|---|---|
| [navi usage and shell scripting](https://github.com/denisidoro/navi/tree/master/docs/usage) center an interactive fuzzy workflow and shell composition | Daily selection should be fast without making scripts depend on a TUI | `myhelp pick` is explicit, can print only a selected topic, and never executes page content; `list`, `search`, and `show` stay non-interactive when piped |
| [cheat editing](https://github.com/cheat/cheat#usage) opens nested sheets in the user's editor | Editor commands such as `code --wait` must work without shell injection or live-file corruption | MyHelp parses Unix and Windows editor command lines, invokes the executable directly, edits a temporary draft, and commits through core revision checks |
| [tealdeer display configuration](https://tealdeer-rs.github.io/tealdeer/config_display.html) provides styled tldr output and an optional pager | A small help CLI needs readable terminal output, raw composition, and paging on every supported OS | MyHelp renders only on a terminal, honors `NO_COLOR`, preserves raw Markdown in pipes, and uses an internal cross-platform pager only on overflow |
| [clap completion generation](https://docs.rs/clap_complete/latest/clap_complete/) supports native shell scripts | Completion should cover the maintained shell matrix without hand-written scripts drifting from the parser | Bash, Fish, Zsh, PowerShell, and Elvish scripts are generated from the live `clap` command definition and smoke-tested |

<!-- markdownlint-enable MD013 MD060 -->

Unlike navi, MyHelp's picker does not insert or run a selected command. Unlike
tealdeer's documented external pager path, which is unavailable on Windows,
MyHelp uses the same internal paging behavior on Linux, macOS, and Windows.
This is a deliberately narrow parity target: comfortable reading, selection,
editing, and shell composition over the same readable vault.

## Explicit non-goals for the MVP

- Executing saved commands.
- Hosting a community cheatsheet registry.
- Building a proprietary cloud synchronization service.
- Replacing tldr clients or the tldr community pages.
- Competing with full snippet/notes workspaces such as massCode.
- Adding a plugin system before the page and adapter contracts are stable.

## Licensing boundary

MyHelp source code is MIT-licensed. Imported tldr community page content is
licensed separately by its authors and repository. Import/export must preserve
source and attribution metadata when required; the application must not silently
relicense imported content.

## Primary references

- [tldr repository and specification](https://github.com/tldr-pages/tldr)
- [tealdeer custom pages](https://tealdeer-rs.github.io/tealdeer/usage_custom_pages.html)
- [navi documentation](https://github.com/denisidoro/navi/tree/master/docs)
- [cheat README](https://github.com/cheat/cheat)
- [pet README](https://github.com/knqyf263/pet)
- [MyHelp foreign-format adapter decision](adr/0002-foreign-format-adapter-levels.md)
- [massCode documentation](https://masscode.io/documentation/)
- [massCode Markdown vault storage](https://masscode.io/documentation/storage.html)
- [Tauri architecture](https://v2.tauri.app/concept/architecture/)
