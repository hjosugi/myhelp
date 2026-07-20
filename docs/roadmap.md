# Roadmap

## Milestone 0: public scaffold

- Shared Rust core.
- Cross-platform Rust CLI with terminal rendering, overflow paging, fuzzy
  selection, JSON output, shell completions, and safe editor arguments.
- Tauri 2 and React editor shell.
- Local Markdown vault.
- Prior-art research and architecture guardrails.
- Cross-platform core/CLI CI.

## Milestone 1: reliable local MVP

- Atomic writes and conflict detection.
- CLI terminal rendering and completions. (complete)
- Desktop create/edit/search/save/rename with recoverable deletion and robust
  dirty-state handling.
- File watching for external edits.
- Accessibility, keyboard navigation, and semantic design tokens.

## Milestone 2: interoperability

- tealdeer zero-copy vault use and collision-safe import/export. (complete)
- tldr validation and format diagnostics. (complete)
- navi lossy import preview plus documented cheat and pet read-only adapter
  boundaries. (complete)
- [Source and license metadata policy](adr/0001-page-metadata-sidecars.md)
  (accepted; implementation follows through adapter issues).

## Milestone 3: distribution

- Checksummed, SBOM-inventoried, provenance-attested CLI archives for Linux,
  macOS, and Windows. (complete)
- Homebrew, Scoop/WinGet, cargo-binstall, and Linux packaging evaluation.
  (complete; promotion requirements are deferred by channel)
- Unsigned desktop package install/remove checks and automatic update policy.
  (checks complete; signing and the updater remain deferred)

## Milestone 4: optional collaboration

- Git-backed vault workflow.
- Sync conflict UI.
- Shareable help packs without a centralized proprietary service.

Features move between milestones only after the associated GitHub issue records
the reason and compatibility impact.
