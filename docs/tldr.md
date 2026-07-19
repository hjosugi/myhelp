# tldr and tealdeer interoperability

MyHelp validates the tldr page structure, imports one existing tldr or
tealdeer page without rewriting it, and exports a vault to a flat tealdeer
custom-page directory. These are format adapters; MyHelp does not execute a
command found in an imported page.

## Commands

```bash
# A vault is not required for validation.
myhelp tldr validate ./git.md
myhelp tldr validate ./git.page.md --topic work/git --json

# Import one page. The destination topic defaults to the source filename.
myhelp --pages-dir ./pages tldr import ./git.page.md
myhelp --pages-dir ./pages tldr import ./git.md --topic work/git

# Export the complete vault and print the deterministic mapping.
myhelp --pages-dir ./pages tldr export ./tealdeer-pages
myhelp --pages-dir ./pages tldr export ./tealdeer-pages --json
```

Validation and unsafe input failures use exit code 5. `--json` writes one
machine-readable validation or conversion report to stdout. Human diagnostics
use `<path>:<line>: <level>[<code>]: <message>`.

## Supported tldr subset

The validator accepts UTF-8 CommonMark pages with:

- one `# <title>` heading as the first line;
- one or more `> <description>` lines before the examples;
- one or more `- <example description>:` lines;
- one nonempty, single-backtick command after each example description; and
- tldr placeholders such as `{{path}}`, `{{stash@{0}}}`, and
  `{{[-A|--all]}}`.

Structural problems are errors and block import or export. Style differences,
including a missing final newline, title/filename mismatch, a missing colon,
or more than eight examples, are warnings and leave the page valid. Every
diagnostic has a stable code and one-based line number.

This is MyHelp's documented interoperability subset, not a replacement for the
upstream repository's complete `tldr-lint` policy. The upstream
[client specification] and [style guide] remain authoritative for community
contributions.

## Import contract

`tldr import` accepts one `.md` upstream page or one `.page.md` tealdeer custom
page. It:

1. rejects symlinks, Windows reparse points, non-files, non-UTF-8 input, and
   pages larger than 1 MiB;
2. validates the page before changing the vault;
3. derives a lowercase topic from the filename unless `--topic` is present;
4. creates a new page without replacing an occupied topic; and
5. preserves the exact UTF-8 bytes, including CRLF line endings.

An adjacent `<name>.page.meta.yaml` source sidecar is copied byte-for-byte.
Alternatively, provenance can be recorded while importing:

```bash
myhelp tldr import ./git.md \
  --page-license CC-BY-4.0 \
  --source-url https://github.com/tldr-pages/tldr \
  --source-title "tldr pages" \
  --source-license CC-BY-4.0 \
  --attribution "tldr-pages contributors"
```

Those options generate an ADR 0001 version-1 sidecar with a new UUID. MyHelp
does not merge flags into an existing source sidecar, because doing so could
silently change provenance. License and attribution values are stored as
provided; the importer does not relicense content or decide whether a license
is sufficient. See the [metadata ADR] for the source/content licensing
boundary.

## Collision-safe flat export

`tldr export` writes `.page.md` files for tealdeer's flat custom-page
directory. Adjacent metadata sidecars are carried to the matching exported
name. Page and metadata bytes are not rewritten.

Export names are deterministic and portable:

1. nested `/` separators and spaces become `-`;
2. names are lowercase;
3. ASCII letters, digits, `.`, `_`, `+`, and `-` remain readable;
4. other UTF-8 bytes use reversible-looking `~xx` escapes;
5. leading dots and Windows reserved names receive a `myhelp-` prefix;
6. long names receive a stable topic hash; and
7. every member of a case-insensitive collision receives a stable
   `--myhelp-<hash>` suffix.

For example, `work/git` normally maps to `work-git.page.md`. Topics
`foo/bar`, `foo-bar`, and `FOO-BAR` each receive distinct hashed filenames
rather than depending on filesystem case sensitivity.

Before creating output files, MyHelp validates every page and preflights every
page and sidecar target, including case-insensitive matches already present in
the destination. It stages and syncs all files, commits them without replacing
anything, and removes its unchanged outputs if a later commit fails. An
occupied name therefore fails the export instead of silently overwriting data.
Use a new or cleared destination for a replacement snapshot.

The JSON report is the conversion manifest. It records the source topic,
exported page and metadata filenames, whether collision resolution was needed,
and validation warnings for every page.

## Zero-copy tealdeer use

A flat MyHelp vault already uses tealdeer's
[custom-page][tealdeer custom pages] `<command>.page.md` filename, so tealdeer
can read it directly. Set an absolute path in tealdeer's `config.toml`:

```toml
[directories]
custom_pages_dir = "/absolute/path/to/myhelp/pages"
```

The [tealdeer directories] manual recommends an absolute path and says
variables are not expanded. Run `tldr --show-paths` to see the paths actually
used by the installed build. Typical current default custom-page locations are:

| Platform | Default |
|---|---|
| Linux | `$XDG_DATA_HOME/tealdeer/pages`, normally `~/.local/share/tealdeer/pages` |
| macOS | `~/Library/Application Support/tealdeer/pages` |
| Windows | `%LOCALAPPDATA%\tealdeer\tealdeer\pages` |

The command output is authoritative because packaging, environment variables,
and future tealdeer versions can change a default. Pointing
`custom_pages_dir` at the MyHelp vault is the zero-copy option. Tealdeer looks
for flat custom pages, so a vault with nested topics should use
`myhelp tldr export` and point tealdeer at that export directory.

Official tldr repository pages use `<command>.md` inside platform and locale
directories. The current MyHelp importer handles those pages one file at a
time, while export targets tealdeer's flat custom-page convention. It does not
claim to generate a complete upstream tldr repository tree.

[client specification]: https://github.com/tldr-pages/tldr/blob/main/CLIENT-SPECIFICATION.md
[style guide]: https://github.com/tldr-pages/tldr/blob/main/contributing-guides/style-guide.md
[metadata ADR]: adr/0001-page-metadata-sidecars.md
[tealdeer custom pages]: https://tealdeer-rs.github.io/tealdeer/usage_custom_pages.html
[tealdeer directories]: https://tealdeer-rs.github.io/tealdeer/config_directories.html
