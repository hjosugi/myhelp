# Localization

Status: English and Japanese are implemented for the desktop editor and for
human-facing CLI messages (issue #11).

MyHelp localizes the text around a user's pages, never the pages themselves.
Page content stays byte-for-byte what the user wrote, and a page's own language
is page metadata, not an interface setting (see below).

## Choosing the language

### Desktop editor

1. The **Language** menu in the top bar is an explicit override. English and
   日本語 are always listed in their own language. The choice is stored in the
   webview's local storage under `myhelp.uiLocale`; choosing **System default**
   removes it.
2. Without an override, the editor walks the operating-system language list
   that the webview exposes (`navigator.languages`) and uses the first entry
   whose primary subtag is supported, so `ja-JP`, `ja`, and `ja_JP` all select
   Japanese. Changing the OS language while MyHelp runs is picked up through
   the `languagechange` event.
3. Anything else falls back to English.

The webview reports the OS language on every desktop platform MyHelp ships:
WebView2 on Windows uses the Windows display language, WKWebView on macOS uses
the preferred-languages list in System Settings, and WebKitGTK on Linux uses the
process locale (`LC_ALL`, `LC_MESSAGES`, `LANG`). The resolved locale is written
to `<html lang>`, so the webview picks Japanese fonts and line-breaking rules
for Japanese text.

### CLI

1. `MYHELP_LANG=en` or `MYHELP_LANG=ja` is an explicit override. Other values
   select English; `auto` or an empty value defers to the next step.
2. The first non-empty POSIX variable among `LC_ALL`, `LC_MESSAGES`, and `LANG`
   decides, as `setlocale` would. `C`, `POSIX`, and unsupported languages select
   English and do **not** fall through to the operating system, so
   `LC_ALL=C myhelp …` is always English.
3. When none of those variables is set, which is common on Windows and in macOS
   GUI launches, the operating-system preference is read through the
   `sys-locale` crate: the Windows user UI language, the macOS preferred
   languages, or the POSIX variables on Linux.
4. Anything else falls back to English.

## What changes with the locale

| Surface | Localized |
|---|---|
| Desktop labels, buttons, placeholders, accessible names | Yes |
| Desktop status bar, confirmations, conflict banner, onboarding | Yes |
| Desktop storage errors | A translated label, followed by the original English detail |
| CLI error line on stderr | Yes; the original English cause chain follows in parentheses |
| CLI `pick` prompt | Yes |
| CLI `--help`, usage errors from `clap` | No |
| CLI stdout: pages, `--raw`, `--json`, `path`, list/search columns, reports | No |
| CLI completion scripts | No |
| Exit codes | No |

Machine-readable output never depends on the locale. Tests run the same
commands under `MYHELP_LANG=en`, `MYHELP_LANG=ja`, `LANG=ja_JP.UTF-8`, and
`LC_ALL=ja_JP.UTF-8` and require identical stdout and exit codes. `--help` stays
English because the completion scripts embed the same help strings, and a
completion script must not change with the shell that generated it.

Error details from the storage layer (paths, topics, operating-system errors)
stay in English after the translated label. They carry the exact data needed to
act on the error, and translating them would lose information.

## Missing translations

- The English catalog is the source: every key exists there.
- A key missing from another catalog falls back to English, then to the key
  name, so the interface never shows an empty label.
- Plural forms follow `Intl.PluralRules` for the locale. Japanese uses only the
  `other` form; if a locale lacks the form it needs, the English form is used.
- An error kind without a label uses `error.unknown`.
- In the CLI, an error MyHelp does not recognize keeps its English text after
  the translated `エラー:` prefix.

The catalog tests fail when a supported locale is missing a key, drops a
placeholder, adds an undocumented placeholder, or defines a key that English
does not have. The Japanese workflow test fails when a visible label, accessible
name, or placeholder in the primary workflow is still in English.

## Japanese text

- The UI font stack keeps Inter first for Latin text and falls back to
  Hiragino Sans, Yu Gothic UI, Meiryo, and Noto Sans CJK JP for Japanese
  glyphs.
- `:lang(ja)` enables strict line breaking (kinsoku shori) and
  `overflow-wrap: anywhere`, so long paths and topics wrap instead of
  overflowing; the status bar wraps long messages in every locale.
- Uppercase eyebrow labels use wide letter spacing only in English.

## Searching mixed-language pages

Search splits the query on Unicode whitespace, including the ideographic space
(U+3000) that Japanese input methods insert, and requires every term to occur
somewhere in the page's topic, title, or content. `git ブランチ` therefore finds
a page that mentions both words anywhere. Full-width ASCII typed through an
input method (`ＧＩＴ`, `ｃａｒｇｏ`) is folded to half-width before
case-insensitive matching. Kana and kanji are compared as written; for example,
half-width katakana is not folded.

## Page language

A page's language is page metadata. The accepted
[page metadata ADR](adr/0001-page-metadata-sidecars.md) (issue #6) reserves the
optional `locale` field of the `<topic>.page.meta.yaml` sidecar for it, as a
BCP 47 tag. MyHelp does not encode page language in folder names or separate
page variants, and the interface language never changes which pages are listed.

## Translator guide

Desktop strings live in [`src/i18n.ts`](../src/i18n.ts):

1. Add the English text to `en` first. Use a dotted key named after where the
   text appears (`conflict.reconcile`, `status.saved`).
2. Put variable parts in `{name}` placeholders instead of joining translated
   fragments, so each language can order words naturally. For markup inside a
   sentence, such as a `<code>` path, render the key with `RichText` and pass
   the element as a slot.
3. Plural messages use `.one` and `.other` suffixes and are rendered with a
   `count`. Provide only the categories the language uses; Japanese needs
   `.other` only.
4. Add the translation to every other catalog. Keep every placeholder. Add a
   placeholder the English text does not use only when it is listed in
   `OPTIONAL_PARAMETERS`: `error.format` may use `{label}`.
5. Write Japanese in the same polite, plain register as the existing strings,
   use full-width punctuation (`、`, `。`, `？`, `（）`), and put a space between
   Japanese and Latin words only where the existing strings do (`{topic} を開く`).
6. Run `pnpm test`. The catalog and workflow tests list any missing or
   malformed translation.

CLI messages live in
[`crates/myhelp-cli/src/i18n.rs`](../crates/myhelp-cli/src/i18n.rs). Add a
message for each new `CliFailure` or core `Error` variant; the `match` is
exhaustive, so the compiler reports a missing one. Never localize stdout, JSON,
or `--help`.

Adding a language means adding it to `SUPPORTED_LOCALES` and `LOCALE_NAMES`
with a complete catalog, and to the CLI `Locale` enum and its parser.
