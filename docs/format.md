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

## Compatibility levels

- A flat `git.page.md` file is intended to be directly readable as a tealdeer
  custom page.
- Nested topics such as `python/new-project.page.md` are a MyHelp organization
  extension. Export to tealdeer must define a collision-safe flat name such as
  `python-new-project.page.md`.
- MyHelp may retain metadata in an index or sidecar later, but it must not make
  the Markdown unreadable without MyHelp.
- Importers for navi, cheat, or pet belong in adapter crates or modules and must
  retain the original source where licensing requires it.

## Topic safety

Topics:

- must be relative;
- cannot contain parent-directory components;
- cannot include the `.page.md` suffix;
- may use `/` to create nested categories.

Symlinks are never followed during recursive scans.

## Open format decisions

The following require ADRs and GitHub issues before implementation:

- tags and aliases;
- stable identifiers across renames;
- source URLs and attribution metadata;
- locale variants;
- executable vs non-executable command examples;
- conflict resolution for external edits.

## References

- [tldr style guide](https://github.com/tldr-pages/tldr/blob/main/contributing-guides/style-guide.md)
- [tldr client specification](https://github.com/tldr-pages/tldr/blob/main/CLIENT-SPECIFICATION.md)
- [tealdeer custom pages and patches](https://tealdeer-rs.github.io/tealdeer/usage_custom_pages.html)
