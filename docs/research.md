# Prior-art research

Snapshot date: 2026-07-18.

MyHelp should be a narrow interoperability layer and editor, not a rewrite of
the mature tools below.

| Project | What it already does well | What MyHelp should reuse or support |
|---|---|---|
| [tldr-pages/tldr](https://github.com/tldr-pages/tldr) | Widely adopted, concise example-oriented Markdown and a published client specification | Use its page structure as the first storage/export contract |
| [tealdeer](https://github.com/tealdeer-rs/tealdeer) | Fast Rust terminal rendering, custom pages and patches, OS-specific custom page locations | Support `<topic>.page.md`; allow a MyHelp vault to be used as tealdeer's custom-page directory |
| [navi](https://github.com/denisidoro/navi) | Fuzzy terminal search, argument placeholders, shell widgets, Git-hosted cheatsheet repositories, tldr/cheat.sh imports | Learn from its search and repository UX; add an adapter instead of copying its execution engine |
| [cheat](https://github.com/cheat/cheat) | Editable layered cheatpaths, tags, syntax highlighting, nested sheets, search | Learn from cheatpath layering and tag UX; keep compatibility behind an adapter |
| [pet](https://github.com/knqyf263/pet) | Focused CLI snippet capture and remote snippet synchronization | Treat Gist/Git synchronization as optional future adapters, not required storage |
| [massCode](https://github.com/massCodeIO/massCode) | Polished desktop editor, local Markdown vault, tags, file watching, broad developer workspace | Learn from its vault/editor UX; keep MyHelp smaller, terminal-first, and tldr-oriented |

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
- [Tauri architecture](https://v2.tauri.app/concept/architecture/)
