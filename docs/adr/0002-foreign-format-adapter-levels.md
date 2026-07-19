# ADR 0002: Honest compatibility levels for navi, cheat, and pet

- Status: Accepted
- Date: 2026-07-19
- Issue: [#5](https://github.com/hjosugi/myhelp/issues/5)

## Context

MyHelp should exchange useful plaintext with established tools without copying
their execution engines or pretending that unlike formats round-trip. The
formats overlap around descriptions, command-shaped text, tags, and
placeholders, but their behavior differs materially:

- navi parses executable snippets, command-backed variable suggestions, and
  inherited contexts;
- cheat displays free-form sheets layered through configured cheatpaths;
- pet stores executable snippets and remote-sync configuration in TOML;
- MyHelp stores non-executable tldr-style Markdown plus optional metadata
  sidecars.

This decision is based on the following upstream revisions so later changes can
be compared against a stable research snapshot:

<!-- markdownlint-disable MD013 MD060 -->

| Tool | Upstream revision | Program license |
|---|---|---|
| navi | [`1ac218c`](https://github.com/denisidoro/navi/tree/1ac218cb1e0e80649ef23c8a916e67efc3086833) (2026-04-13) | [Apache-2.0](https://github.com/denisidoro/navi/blob/1ac218cb1e0e80649ef23c8a916e67efc3086833/LICENSE) |
| cheat | [`b8098dc`](https://github.com/cheat/cheat/tree/b8098dc1b9de846d76e68102e03aeb0f918bb0da) (2026-02-16) | [MIT](https://github.com/cheat/cheat/blob/b8098dc1b9de846d76e68102e03aeb0f918bb0da/LICENSE.txt) |
| pet | [`3661dc9`](https://github.com/knqyf263/pet/tree/3661dc9cb080bf48343e2d7f56a75abd3864c1d6) (2026-03-13) | [MIT](https://github.com/knqyf263/pet/blob/3661dc9cb080bf48343e2d7f56a75abd3864c1d6/LICENSE) |

<!-- markdownlint-enable MD013 MD060 -->

The license of an application does not determine the license of user-authored
or third-party cheatsheet content.

## Decision

Foreign formats stay behind a `myhelp-core` adapter trait and return one typed
conversion report. Storage and adapters remain separate: inspecting a foreign
file does not create a vault, write a page, parse general MyHelp metadata, or
start a process.

Every report states:

- the source format and path;
- the proposed topic;
- the compatibility level;
- that the operation is a dry run;
- whether the preview is convertible and whether it is lossless;
- source tags retained only in the report;
- the generated page, when one is honest;
- line-oriented field diagnostics classified as mapped, reported-only, or
  unsupported.

An adapter must use the lowest honest compatibility level:

1. **lossy import preview** when it can generate useful MyHelp Markdown while
   reporting every material loss;
2. **read-only index** when content can be discovered but automatic conversion
   would invent structure;
3. **unsupported** when even a read-only interpretation is ambiguous or unsafe.

No navi, cheat, or pet export is claimed by this prototype. Export would lose
MyHelp metadata, and navi or pet would treat exported command text as
executable selections. Any future export needs an explicit user action,
metadata-loss policy, and a separate security review.

## Format decisions

### navi: lossy import preview

The implemented prototype accepts one `.cheat` or `.cheat.md` file with one
`%` context and described, single-line snippets:

<!-- markdownlint-disable MD013 MD060 -->

| navi field | MyHelp preview | Disposition |
|---|---|---|
| `# Description` | `- Description:` | Mapped |
| one-line snippet | one backtick-wrapped tldr example | Mapped as non-executable text |
| `<name>` where the name is alphanumeric or `_` | `{{name}}` | Mapped |
| `% tag, tag` | `sourceTags` in the report | Reported only until general metadata support lands |
| `$ name: command` and its finder options | Diagnostic only | Unsupported; never executed |
| `@ inherited, context` | Diagnostic only | Unsupported; never resolved |
| `;` metacomment | Diagnostic only | Unsupported; navi itself ignores it |
| multiline or backtick-containing snippet | Diagnostic and a non-convertible report | Unsupported by the current tldr subset |
| multiple `%` contexts in one file | Diagnostic and a non-convertible report | Unsupported because inventing destination topic names would be lossy |

<!-- markdownlint-enable MD013 MD060 -->

The generated description says that the page is for display only. Formatting,
source ordering, and tags do not round-trip, so every navi report is explicitly
lossy even when it is convertible.

The parser follows the documented syntax and the upstream parser behavior
relevant to boundaries: `%` begins a context, `#` begins a described item, `$`
defines a command-backed suggestion, `@` inherits a context, and fenced or
ordinary snippets may span lines. See the pinned
[syntax documentation](https://github.com/denisidoro/navi/blob/1ac218cb1e0e80649ef23c8a916e67efc3086833/docs/cheatsheet/syntax/README.md)
and [parser](https://github.com/denisidoro/navi/blob/1ac218cb1e0e80649ef23c8a916e67efc3086833/src/parser.rs).

### cheat: read-only index first

cheat sheets are free-form text. Their optional YAML front matter has only
`tags` and `syntax`; cheatpath configuration supplies a name/path, more tags,
and a `readonly` flag. The tool merges and sorts sheet and cheatpath tags.

The future smallest useful adapter is a read-only index:

- filename/title becomes a candidate MyHelp topic;
- merged tags become candidate sidecar tags;
- syntax becomes adapter-specific metadata, not a language invented in the
  Markdown body;
- cheatpath identity and read-only state are provenance and source policy;
- the body stays a read-only preview unless it independently validates as the
  supported tldr subset.

Automatic general import would need to infer descriptions and command blocks
from unconstrained text, so it is not honest. Export cannot preserve arbitrary
MyHelp metadata or reconstruct cheatpath layering. These conclusions follow
the pinned
[sheet model](https://github.com/cheat/cheat/blob/b8098dc1b9de846d76e68102e03aeb0f918bb0da/internal/sheet/sheet.go),
[front-matter parser](https://github.com/cheat/cheat/blob/b8098dc1b9de846d76e68102e03aeb0f918bb0da/internal/sheet/parse.go),
and
[cheatpath model](https://github.com/cheat/cheat/blob/b8098dc1b9de846d76e68102e03aeb0f918bb0da/internal/cheatpath/cheatpath.go).

### pet: read-only index before per-snippet previews

pet stores a list of TOML records with `Description`, multiline `Command`,
`Tag`, and `Output`. A future per-snippet preview could map a description and a
single-line command to one tldr example and map `<name>` to `{{name}}`.
However:

- pet permits multiline commands;
- `<name=default_value>` has no tldr equivalent;
- output has no field in the supported MyHelp body;
- tags need general sidecar support;
- a file can hold many snippets without stable MyHelp topic names;
- snippet directories and filenames describe source grouping, not page
  semantics.

The first honest implementation is therefore a read-only index. A later dry-run
preview may require an explicit grouping/topic strategy, with default values
and output reported as losses.

pet's backend, access tokens, Gist/GitLab/GitHub Enterprise identifiers,
visibility, TLS, and auto-sync fields are operational configuration rather than
snippet metadata. MyHelp will never import secrets or contact a backend while
inspecting content. Remote `UpdatedAt` and local modification time are sync
state and also remain outside the page model. See the pinned
[snippet model](https://github.com/knqyf263/pet/blob/3661dc9cb080bf48343e2d7f56a75abd3864c1d6/snippet/snippet.go),
[configuration model](https://github.com/knqyf263/pet/blob/3661dc9cb080bf48343e2d7f56a75abd3864c1d6/config/config.go),
and
[sync model](https://github.com/knqyf263/pet/blob/3661dc9cb080bf48343e2d7f56a75abd3864c1d6/sync/sync.go).

## Security and filesystem boundary

Adapter inspection:

- reads at most 1 MiB of UTF-8;
- requires a regular source file;
- rejects a source symlink or Windows reparse point and opens with the existing
  no-follow helper;
- never reads navi variable commands, cheat configuration, or pet backend
  configuration as instructions;
- never invokes a shell, subprocess, network client, or sync provider;
- never writes a page or metadata sidecar.

`myhelp adapter inspect` is handled before vault discovery. A missing proposed
vault therefore remains missing.

## Fixture licensing

Current adapter fixtures are original MyHelp test data and are covered by this
repository's MIT license. The fixture directory records that provenance.

If a future test copies upstream program fixtures, it must retain the upstream
license and notices. If it copies community or user cheatsheets, their content
license and attribution must be recorded separately; the program license alone
is insufficient.

## Consequences

- Users get a useful navi conversion preview without hidden execution or
  filesystem writes.
- Unsupported navi behavior is visible in both human and JSON reports.
- cheat and pet are not overclaimed; their next adapters have bounded,
  reviewable scopes.
- General metadata support can later promote reported tags into sidecars
  without changing the foreign parser or report contract.
- Applying previews, splitting multi-context files, and exporting executable
  formats remain separate issues rather than accidental scope growth.
