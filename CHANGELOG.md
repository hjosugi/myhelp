# Changelog

<!-- markdownlint-disable MD024 -->

All notable user-visible changes to MyHelp are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
MyHelp is still pre-1.0, so minor versions may change public behavior.

## [Unreleased]

## [0.7.0] - 2026-07-19

### Added

- Add tested CLI release archives for Linux x64, macOS Apple Silicon, and
  Windows x64 with stable target-based names.
- Add clean archive install/use/uninstall checks, native desktop bundle
  install/uninstall checks, SHA-256 checksums, an SPDX JSON source SBOM, and
  GitHub artifact provenance.
- Add a gated tag workflow that assembles a draft, uploads the complete asset
  set, and only then publishes a prerelease.
- Add cargo-binstall metadata matching the tested CLI archive layout.
- Document the release decision, signing-key threat model, channel lifecycle,
  rollback boundary, and maintainer release procedure.

### Changed

- Keep CLI archives and desktop bundles visibly separate. Desktop filenames
  retain an `unsigned` marker until operating-system signing and notarization
  are deployed.
- Treat only channels whose exact artifact and removal path pass CI as
  advertised channels.

### Fixed

- Reject release archives with extra entries, tampered or malformed checksum
  manifests, internal dependency version drift, and invalid SemVer
  prerelease identifiers.
- Connect release tags to the native build, assembly, attestation, draft
  verification, and prerelease publication jobs instead of leaving the
  release contract as unused tooling.

### Security

- Pin the SBOM, artifact-download, provenance-attestation, and Nix installer
  actions to reviewed commit SHAs.
- Disable package and compiler caches in the workflow that also publishes tags
  so a release cannot restore runtime state written by a less-trusted run.
- Keep signing credentials out of pull-request and ordinary `main` jobs;
  publishing receives only scoped GitHub permissions after every build and
  smoke check succeeds.

### Distribution status

This is a prerelease. The CLI archives are the first tested binary download
channel. Native desktop bundles are attached as explicitly unsigned evaluation
artifacts and may trigger operating-system warnings; they are not yet an
advertised desktop installation channel.

## [0.6.0] - 2026-07-19

### Added

- Add a safe, typed navi conversion preview and honest compatibility boundaries
  for navi, cheat, and pet.

## [0.5.0] - 2026-07-19

### Added

- Add loss-aware tldr validation, import, export, and tealdeer interoperability.

## [0.4.0] - 2026-07-19

### Added

- Add terminal rendering, paging, fuzzy selection, JSON output, completions,
  portable editor parsing, and stable exit behavior.

## [0.3.0] - 2026-07-19

### Added

- Add the full desktop editor workflow, semantic design tokens, keyboard and
  screen-reader behavior, recoverable deletion, and vault switching.

## [0.2.2] - 2026-07-19

### Added

- Add unsigned native desktop packaging smoke builds on Linux, macOS, and
  Windows.

## [0.2.1] - 2026-07-19

### Added

- Add the desktop CSP, narrow Tauri capabilities, inert Markdown rendering,
  resource limits, and desktop threat model.

## [0.2.0] - 2026-07-19

### Added

- Add revision-aware atomic storage, conflict recovery, vault watching, and
  cross-platform link and traversal defenses.

## [0.1.1] - 2026-07-19

### Added

- Add the original MyHelp brand mark, generated native icons, and asset checks.

## [0.1.0] - 2026-07-19

### Added

- Add the initial local-first Rust core, CLI, Tauri desktop scaffold, Nix
  development environment, and plaintext format contract.

[Unreleased]: https://github.com/hjosugi/myhelp/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/hjosugi/myhelp/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/hjosugi/myhelp/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/hjosugi/myhelp/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/hjosugi/myhelp/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/hjosugi/myhelp/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/hjosugi/myhelp/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/hjosugi/myhelp/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/hjosugi/myhelp/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/hjosugi/myhelp/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/hjosugi/myhelp/releases/tag/v0.1.0
