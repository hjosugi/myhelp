# Prior-art research

Snapshot date: 2026-07-18.

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
- [massCode documentation](https://masscode.io/documentation/)
- [massCode Markdown vault storage](https://masscode.io/documentation/storage.html)
- [Tauri architecture](https://v2.tauri.app/concept/architecture/)
