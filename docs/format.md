# Page format contract

## Initial format

MyHelp pages use UTF-8 Markdown and the tealdeer custom-page filename:

```text
<topic>.page.md
```

The initial content conventions follow tldr pages:

```markdown
# Topic title

> A short explanation.
> A second context line when useful.
> Official documentation: <https://example.com/docs>.

- Describe one practical action:

`command --option {{placeholder}}`
```

An example description may be followed by more than one command when they are
alternative spellings of the same action, or two halves of a workflow that read
better apart:

```markdown
- Update one layer at a time:

`task update:nix`

`task update:rust`
```

tealdeer renders these, so validation accepts them and counts the block as a
single example. A command with no example description above it at all is still
reported as `command-without-description`.

## Compatibility levels

- A flat `git.page.md` file is intended to be directly readable as a tealdeer
  custom page.
- Nested topics such as `python/new-project.page.md` are a MyHelp organization
  extension. Export to tealdeer uses a deterministic collision-safe flat name.
- MyHelp metadata is optional and lives beside the Markdown as
  `<topic>.page.meta.yaml`. The accepted
  [metadata ADR](adr/0001-page-metadata-sidecars.md) keeps the page body
  unchanged.
- Importers for navi, cheat, or pet belong in adapter crates or modules and must
  retain the original source where licensing requires it.
- Foreign adapters declare a lossy-preview, read-only-index, or unsupported
  compatibility level. The navi prototype is dry-run only; cheat and pet remain
  read-only-index decisions until their grouping and metadata policies are
  implemented.

## Optional metadata

`git.page.md` can be accompanied by `git.page.meta.yaml`. The sidecar may hold a
stable ID, tags, aliases, locale, license, and provenance. A missing, invalid,
or newer sidecar never makes readable Markdown disappear.

MyHelp-managed moves carry both files and preserve the ID. Direct
tldr/tealdeer use continues to consume only `.page.md`. Foreign-format exports
must report fields they cannot represent and cannot silently discard
attribution or license data.

## tldr and tealdeer conversion

MyHelp validates a documented tldr subset before import or export. Import keeps
UTF-8 page bytes unchanged. Flat export lowercases and escapes topics, handles
Windows reserved filenames and length bounds, and gives every member of a
case-insensitive collision a stable content-hash suffix. Existing targets are
never replaced.

The exact commands, supported syntax, mapping algorithm, provenance options,
and tealdeer zero-copy paths are documented in the
[tldr and tealdeer interoperability contract](tldr.md).

## Topic safety

Topics:

- must be relative;
- cannot contain parent-directory components;
- cannot include the `.page.md` suffix;
- may use `/` to create nested categories.

Symlinks and Windows reparse points are rejected during scans and direct
read/write access.

Topics are limited to 240 UTF-8 bytes and page files to 1 MiB. These bounds
keep IPC, path handling, preview rendering, and externally added files
predictable across supported platforms. An oversized external page remains an
ordinary readable file to other tools, but MyHelp reports it instead of loading
or overwriting it.

## Conflict copies

When a page changes on disk after it was read, MyHelp leaves that disk version
untouched and can preserve the caller's draft beside it:

```text
git.page.md
git.page.conflict-<content-sha256>.md
```

Conflict copies are ordinary UTF-8 Markdown. Their deterministic content hash
avoids duplicate copies for repeated save attempts, and their names do not end
in `.page.md`, so they are not normal pages or tealdeer custom pages. A user may
compare, rename, or delete them with any file tool.

## Recoverable deletions

Desktop deletion does not permanently unlink a page. Core moves it beside the
original topic:

```text
git.page.deleted-<content-sha256>.md
git.page.deleted-<content-sha256>-1.md
```

The numeric suffix is used only when that recovery filename already exists.
These are ordinary UTF-8 Markdown files, but they do not end in `.page.md` and
therefore do not appear as normal pages or tealdeer custom pages. Undo restores
the exact file only while `git.page.md` is unoccupied.

When an optional metadata sidecar exists, managed rename, deletion, and restore
carry it without parsing or rewriting it. A deleted sidecar is named after the
recovery file, for example
`git.page.deleted-<content-sha256>.page.meta.yaml`.

## Open format decisions

The following still require ADRs and GitHub issues before implementation:

- executable versus non-executable command examples;

## References

- [tldr style guide](https://github.com/tldr-pages/tldr/blob/main/contributing-guides/style-guide.md)
- [tldr client specification](https://github.com/tldr-pages/tldr/blob/main/CLIENT-SPECIFICATION.md)
- [tealdeer custom pages and patches](https://tealdeer-rs.github.io/tealdeer/usage_custom_pages.html)
- [MyHelp tldr and tealdeer adapter](tldr.md)
- [MyHelp metadata ADR](adr/0001-page-metadata-sidecars.md)
- [MyHelp foreign-format adapters](adapters.md)
- [Foreign-format compatibility ADR](adr/0002-foreign-format-adapter-levels.md)
