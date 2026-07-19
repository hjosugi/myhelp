# Foreign-format adapter contract

MyHelp inspects foreign cheatsheet formats as untrusted text. The current
prototype generates a navi-to-MyHelp preview without writing to the vault:

```bash
myhelp adapter inspect navi ./git.cheat
myhelp adapter inspect navi ./git.cheat --topic tools/git
myhelp adapter inspect navi ./git.cheat --json
```

The source filename must end in `.cheat` or `.cheat.md`. `--topic` overrides
only the proposed destination topic.

## Report behavior

The command always starts as a dry run:

- it does not discover or create a vault;
- it does not write the generated page;
- it never executes navi snippets or `$ name: command` variable sources;
- it reads one regular, non-symlink UTF-8 file of at most 1 MiB;
- it emits every mapped, reported-only, and unsupported field as a typed
  diagnostic.

A convertible report exits with code 0. A report with an error still prints the
complete human or JSON report and exits with code 5. Filesystem failures use the
normal CLI runtime or invalid-data exit rules.

The JSON shape is stable for the prototype:

```json
{
  "format": "navi",
  "compatibility": "lossyImportPreview",
  "dryRun": true,
  "sourcePath": "git.cheat",
  "topic": "git",
  "convertible": true,
  "lossless": false,
  "sourceTags": ["git", "workflow"],
  "generatedPage": "# Git\n...",
  "diagnostics": [
    {
      "level": "warning",
      "line": 7,
      "code": "dynamic-variable-source-omitted",
      "sourceField": "variable.branch.source",
      "disposition": "unsupported",
      "message": "..."
    }
  ]
}
```

`generatedPage` is a preview, not proof of round-trip compatibility.
`sourceTags` are retained in the report but are not written into MyHelp
metadata until the general metadata implementation is available.

## Supported navi subset

One `%` context may contain one or more `#` descriptions followed by
single-line snippets. `<name>` placeholders map to `{{name}}`. The generated
page follows MyHelp's supported tldr subset and states that commands are display
only.

The report rejects or flags:

- multiple `%` contexts;
- multiline snippets;
- snippets containing backticks;
- missing descriptions or contexts;
- unclosed Markdown code fences;
- unsupported `@` context inheritance;
- omitted `$` dynamic variable sources and options;
- ignored `;` metacomments;
- noncanonical prefix spacing and invalid placeholder names.

Warnings may still produce a convertible preview when the remaining page is
useful. Errors make `convertible` false. Nothing in either case is applied
automatically.

## Other formats

cheat and pet are currently documented research targets, not implemented
parsers. Their honest first compatibility level is a read-only index because
cheat bodies are unstructured and pet files require grouping multiple snippets
while retaining output, defaults, tags, and provenance.

The complete field mapping, upstream revision pins, licensing boundary, and
future compatibility levels are in
[ADR 0002](adr/0002-foreign-format-adapter-levels.md).
