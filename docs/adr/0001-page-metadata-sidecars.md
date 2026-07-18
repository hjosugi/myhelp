# ADR 0001: Optional page metadata sidecars

- Status: Accepted
- Date: 2026-07-19
- Issue: [#6](https://github.com/hjosugi/myhelp/issues/6)

## Context

MyHelp pages must remain ordinary tldr-style Markdown that a text editor,
script, or tealdeer can read without MyHelp. Interoperability and reliable
editing also need structured data that the tldr page body cannot represent:

- stable identity across MyHelp-managed renames;
- tags and alternate lookup names;
- page locale;
- source, attribution, and license information for imported content;
- future adapter-specific data.

Adjacent tools make different tradeoffs. tldr keeps locale and platform in the
directory layout. Tealdeer consumes `<name>.page.md` files directly. Navi uses
format-specific tag and variable lines, cheat permits YAML front matter, pet
stores snippets and tags in TOML, and massCode stores metadata in Markdown
front matter. MyHelp cannot adopt any one of those containers without either
losing fields or weakening zero-copy tldr/tealdeer compatibility.

## Decision

Metadata is an optional YAML 1.2 sidecar adjacent to its Markdown page:

```text
python/new-project.page.md
python/new-project.page.meta.yaml
```

The sidecar name is produced by replacing `.page.md` with `.page.meta.yaml`.
The Markdown remains the source of truth for the title, explanation, and
examples. A sidecar augments a page; it never contains or replaces the page
body.

A vault with no sidecars is a complete, valid MyHelp vault. Reading a page must
not create metadata as a side effect.

## Version 1 document

```yaml
schema_version: 1
id: 550e8400-e29b-41d4-a716-446655440000
tags:
  - git
  - workflow
aliases:
  - git-new-branch
locale: en
license: MIT
sources:
  - url: https://github.com/tldr-pages/tldr
    title: tldr pages
    license: CC-BY-4.0
    attribution: tldr-pages contributors
```

The version 1 fields are:

<!-- markdownlint-disable MD013 MD060 -->

| Field | Required in a sidecar | Contract |
|---|---|---|
| `schema_version` | Yes | Positive integer; version 1 is defined here |
| `id` | Yes | Canonical lowercase UUID; new IDs use UUID v4 |
| `tags` | No | Ordered, duplicate-free list of non-empty UTF-8 labels |
| `aliases` | No | Ordered, duplicate-free list of topic-shaped alternate names |
| `locale` | No | Canonical BCP 47 language tag such as `en`, `ja`, or `pt-BR` |
| `license` | No | SPDX license expression for the page as stored |
| `sources` | No | Ordered list of provenance records |

<!-- markdownlint-enable MD013 MD060 -->

Each source record has an absolute `url` and may have `title`, `license`, and
`attribution`. A known source license uses an SPDX expression. A license that
does not have an SPDX identifier uses `LicenseRef-*` and must include a source
URL or attribution that lets a user find its terms.

Empty lists are equivalent to omitted `tags`, `aliases`, or `sources`. Writers
omit empty optional fields to keep diffs small.

## Identity and lookup

The topic remains the relative path without `.page.md`; it is not duplicated in
the sidecar.

An ID is unique within a vault. MyHelp-managed rename and move operations carry
the sidecar with the page and preserve the ID. Duplicate IDs are conflicts and
must not be repaired automatically.

Aliases follow the same path-safety rules as topics but do not create files.
Canonical topics take precedence over aliases. Two pages claiming the same
alias, or an alias equal to another page's canonical topic, produce a
diagnostic instead of an arbitrary winner.

An external file manager can move only the Markdown or only its sidecar. MyHelp
reports the remaining sidecar as orphaned and treats the moved Markdown as an
unmanaged page. It must not guess an association from similar names or content.
A later recovery command may let the user reattach the sidecar explicitly.

## Missing, invalid, and newer metadata

Metadata failure must never make readable Markdown disappear:

- Missing sidecar: return the page with `MetadataState::Missing`.
- Valid supported sidecar: return parsed metadata and preserved unknown fields.
- Invalid YAML or invalid known fields: return the page with diagnostics and
  `MetadataState::Invalid`.
- Newer `schema_version`: return the page with
  `MetadataState::UnsupportedVersion`.
- Sidecar symlink: refuse to read or write it and return a security diagnostic.

Page content may still be read and edited while metadata is invalid or newer,
because those writes do not touch the sidecar. Metadata mutation, managed
rename, and metadata-dependent export are refused until the problem is
resolved.

## Versioning and unknown fields

`schema_version` is the major version of the sidecar contract. Adding an
optional field with a safe default does not require a new major version.
Removing a field, changing its meaning, or changing a field type does.

A version 1 reader:

- ignores unknown fields when interpreting metadata;
- preserves their parsed keys and values when rewriting a supported document;
- warns for an unknown non-`x-` field;
- does not promise to preserve YAML comments, key order, anchors, or scalar
  styling;
- refuses to rewrite a document with a newer major version.

Third-party extensions use an `x-<owner>-<name>` top-level key. YAML custom
tags, anchors, aliases, merge keys, duplicate mapping keys, and non-string
mapping keys are outside the supported subset. This keeps documents portable
and avoids surprising expansion or type coercion.

## Core API boundary

`myhelp-core` owns sidecar paths, parsing, validation, diagnostics, and writes.
The intended public shape is:

```rust
pub struct Page {
    pub topic: String,
    pub title: String,
    pub content: String,
    pub path: PathBuf,
    pub metadata: MetadataState,
}

pub enum MetadataState {
    Missing,
    Valid(PageMetadata),
    Invalid(Vec<MetadataDiagnostic>),
    UnsupportedVersion { found: u32 },
}

pub struct PageMetadata {
    pub schema_version: u32,
    pub id: PageId,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub locale: Option<LanguageTag>,
    pub license: Option<LicenseExpression>,
    pub sources: Vec<PageSource>,
    pub extensions: MetadataExtensions,
}
```

These names express the boundary, not a commitment to a particular YAML, UUID,
language-tag, or SPDX Rust crate. CLI and Tauri code consume serializable core
types and diagnostics. Neither layer parses YAML or constructs sidecar paths.

## Import and export

Native MyHelp copy/export preserves the Markdown and sidecar together.

Foreign-format adapters return an explicit conversion report with fields that
were preserved, mapped, or unsupported:

- tldr/tealdeer receives unchanged compatible Markdown. Locale maps to the tldr
  language directory only when the adapter has an unambiguous BCP 47 to POSIX
  locale mapping.
- cheat may receive tags in its front matter.
- navi and pet receive only fields their formats can represent.
- imported content receives a new MyHelp ID while retaining source, license,
  and attribution in the sidecar.

An export must not silently discard source, attribution, or license data.
Metadata-aware exports copy a sidecar or emit a separate manifest. A
foreign-format-only export blocks on material metadata loss unless the user
explicitly requests a lossy export; its conversion report remains available
without writing into the foreign page body.

## Migration

Pages without sidecars require no migration. The first operation that needs
metadata creates a version 1 sidecar; opening or listing a page does not.

Future migrations:

1. scan and report affected sidecars without changing them;
2. require an explicit migration command;
3. write same-directory temporary files and atomically replace each sidecar;
4. retain readable backups until the migration completes;
5. be restartable and report partial completion;
6. never rewrite Markdown merely to migrate metadata.

Multi-file page and sidecar operations use the atomic-write and conflict
contract from issue #1. No database or hidden migration state is required.

## Security and portability

- Sidecars are readable UTF-8 files and may be versioned or synced with the
  Markdown vault.
- Scans never follow page or sidecar symlinks.
- Metadata paths use the same absolute-path and parent-traversal rejection as
  topics.
- Parsers impose document, collection, and scalar size limits.
- URLs and attribution are inert text. They do not trigger network access.
- Metadata never marks a saved command as trusted or executable.
- Writers use `\n`; readers accept `\n` and `\r\n`.

## Consequences

Benefits:

- Existing Markdown remains byte-for-byte usable by tldr-style tooling.
- A metadata-only edit produces a focused Git diff.
- One malformed page cannot corrupt a vault-wide index.
- Older MyHelp versions can keep editing Markdown they understand.
- Imported licensing data has a durable place outside the MIT-licensed app.

Costs:

- A managed page may consist of two files.
- External moves can orphan a sidecar.
- Vault-wide alias and ID uniqueness requires an index built by core.
- MyHelp needs explicit conversion reports because foreign formats represent
  different subsets.

## Rejected alternatives

### YAML front matter in every Markdown page

Cheat and massCode demonstrate that front matter is practical, but it changes
the tldr-style body and can be rejected or rendered as content by consumers
that expect the page to begin with its Markdown heading. It also rewrites
third-party content merely to attach MyHelp metadata.

### One vault index

A single index makes every tag or rename touch one file, creates a sync and Git
merge hotspot, and lets one malformed document affect the whole vault.

### Markdown-only conventions

Visible prose can carry source links, but it cannot provide stable typed IDs,
unknown-field versioning, or reliable adapter metadata without inventing a new
page syntax.

### Required database

A database would weaken manual editability, tldr/tealdeer zero-copy use, Git
diff quality, and provider-neutral folder sync.

## References

- [tldr client specification](https://github.com/tldr-pages/tldr/blob/main/CLIENT-SPECIFICATION.md)
- [tealdeer custom pages](https://tealdeer-rs.github.io/tealdeer/usage_custom_pages.html)
- [navi cheatsheet syntax](https://github.com/denisidoro/navi/tree/master/docs/cheatsheet/syntax)
- [cheat cheatsheets](https://github.com/cheat/cheat#cheatsheets)
- [pet snippet storage](https://github.com/knqyf263/pet/blob/main/snippet/snippet.go)
- [massCode Markdown vault](https://masscode.io/documentation/storage.html)
- [YAML 1.2.2](https://yaml.org/spec/1.2.2/)
- [BCP 47 / RFC 5646](https://www.rfc-editor.org/rfc/rfc5646)
- [SPDX license expressions](https://spdx.github.io/spdx-spec/v3.0.1/annexes/spdx-license-expressions/)
