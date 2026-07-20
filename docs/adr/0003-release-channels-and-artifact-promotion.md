# ADR 0003: Test artifacts before promoting release channels

- Status: Accepted
- Date: 2026-07-19
- Issue: [#8](https://github.com/hjosugi/myhelp/issues/8)

## Context

MyHelp has one source tree but two materially different products:

- the portable `myhelp` CLI, which can be built from source or unpacked as one
  binary;
- the Tauri desktop app, whose native bundles inherit each operating system's
  installer, signing, reputation, and update requirements.

Treating these as interchangeable release assets would hide important trust
boundaries. An unsigned CLI archive with a published digest is useful to an
early adopter. An unsigned desktop installer may trigger Gatekeeper or
SmartScreen and is not a credible default installation channel. A store
manifest is not verified merely because its JSON or formula parses.

The release also needs to stay local-first. Installing, updating, or removing
the application must not migrate, upload, or delete the user's Markdown vault.

## Decision

An artifact moves through three explicit states:

1. **build candidate**: produced on a native CI runner with read-only source
   access;
2. **tested artifact**: its exact archive or bundle has passed the documented
   clean install, basic-use where applicable, and uninstall check;
3. **advertised channel**: documentation may present it as an installation
   choice only after the exact artifact shape is tested continuously.

Git tags and GitHub Releases do not promote a failing candidate. Tag CI builds
the complete set, runs the same tests used by pull requests, generates an SPDX
JSON source SBOM and `SHA256SUMS`, attests the artifacts, and stages a draft
release. The draft is published as a prerelease only after every dependency
job succeeds.

Published tags and assets are append-only. A bad release is followed by a new
patch version; maintainers do not retarget its tag or replace assets in place.
The affected release notes identify the fault and the last known-good version.

## Artifact contract

CLI archives and desktop bundles use different prefixes and are never placed
in one archive.

<!-- markdownlint-disable MD013 MD060 -->

| Product | Tested host | Rust target | Release filename |
|---|---|---|---|
| CLI | Ubuntu x64 | `x86_64-unknown-linux-gnu` | `myhelp-cli-x86_64-unknown-linux-gnu-vVERSION.tgz` |
| CLI | macOS Apple Silicon | `aarch64-apple-darwin` | `myhelp-cli-aarch64-apple-darwin-vVERSION.tgz` |
| CLI | Windows x64 | `x86_64-pc-windows-msvc` | `myhelp-cli-x86_64-pc-windows-msvc-vVERSION.tgz` |
| Desktop candidate | Ubuntu x64 | `x86_64-unknown-linux-gnu` | `myhelp-desktop-x86_64-unknown-linux-gnu-vVERSION-deb-unsigned.deb` |
| Desktop candidate | macOS Apple Silicon | `aarch64-apple-darwin` | `myhelp-desktop-aarch64-apple-darwin-vVERSION-dmg-unsigned.dmg` |
| Desktop candidate | Windows x64 | `x86_64-pc-windows-msvc` | `myhelp-desktop-x86_64-pc-windows-msvc-vVERSION-nsis-unsigned.exe` |

<!-- markdownlint-enable MD013 MD060 -->

Every CLI archive has one target/version-named root containing `myhelp` (or
`myhelp.exe`), `LICENSE`, and `README.md`. This matches the explicit
`cargo-binstall` metadata in the CLI manifest. Binstall remains unadvertised
until the crate discovery and archive-signature path are tested end to end.

Desktop bundles retain `unsigned` in their public filename until the
platform-specific controls in the
[signing-key threat model](../release-signing.md) are deployed. Attaching an
unsigned bundle to an early prerelease makes native evaluation reproducible;
it does not make that bundle an advertised desktop installation channel.

## Channel decisions

<!-- markdownlint-disable MD013 MD060 -->

| Channel | Decision | Promotion requirement |
|---|---|---|
| GitHub CLI archives | Advertised preview from v0.7.0 | Native build; unpack/use/remove smoke; checksum; SBOM; provenance |
| Nix Flake CLI app | Existing source-build preview | `flake check` and `nix run` on Linux and macOS CI |
| `cargo install --git` | Documented source-build fallback | Clean `cargo install --path` equivalent on all Rust runners; locked dependencies |
| crates.io `cargo install` | Deferred | Publish `myhelp-core` then `myhelp-cli`; install/uninstall test from crates.io |
| `cargo binstall` | Metadata prepared, channel deferred | Registry discovery plus signed-archive install/uninstall CI |
| Homebrew formula/cask | Deferred | Hosted formula/cask, audit/test, checksum updates, and signed macOS desktop bundle for cask |
| Scoop | Deferred | Manifest validation plus clean Windows Sandbox install/uninstall |
| WinGet | Deferred | `winget validate`, Windows Sandbox test, and signed installer |
| Linux `deb` | Unsigned evaluation artifact | Package install/remove tested; signing/repository policy still required for an advertised channel |
| AppImage | Deferred | Native build, executable smoke, signing, update and rollback decision |
| Flatpak/distro repositories | Deferred | Maintained manifest/spec, sandbox review, repository acceptance, clean install/uninstall test |
| Tauri updater | Deferred | Mandatory updater signing key, OS code signing, authenticated manifest, staged rollout, and tested recovery |

<!-- markdownlint-enable MD013 MD060 -->

The first matrix intentionally follows available GitHub-hosted native runners.
Adding x64 macOS, ARM Linux, ARM Windows, musl, MSI, RPM, or AppImage requires a
new matrix entry and the same candidate-to-channel promotion evidence. A
successful cross-compile alone is insufficient.

## Version and changelog policy

The core, CLI, desktop crate, Tauri config, frontend package, Nix package, and
both Rust lockfiles share one version. `scripts/release.mjs validate` rejects
drift and requires a dated `CHANGELOG.md` entry before a release tag can pass.

MyHelp follows Semantic Versioning:

- while `0.y.z`, a minor version may change public CLI, IPC, file-format, or
  adapter behavior and must document migration or compatibility impact;
- a patch version fixes or safely refines behavior without intentionally
  breaking the documented contract;
- `1.0.0` will mark the first stable public contract;
- prerelease identifiers are allowed for explicit test cohorts and never
  silently replace a stable release.

Every changelog entry separates added, changed, fixed, removed, security, and
distribution impact as applicable. Release notes are extracted from that exact
version section so the repository remains the source of truth.

## Integrity, provenance, and reproducibility

`SHA256SUMS` detects accidental or malicious asset substitution when obtained
through an independent trusted path. GitHub artifact attestations bind each
published asset digest to the tag workflow and can be checked with
`gh attestation verify`. Neither control replaces operating-system code signing.

The source SBOM is generated from the checked-out, locked source tree in SPDX
JSON. It is useful for dependency inventory and incident response but is not a
claim that every native framework is discoverable inside every installer.

Dependencies and action revisions are pinned, but native toolchains and hosted
runner images are not bit-for-bit pinned. Releases therefore claim traceable,
locked builds, not reproducible binaries. Bit-for-bit reproducibility requires
fixed toolchain images, normalized archive metadata, and independent rebuild
comparison before that language is used.

## Install, update, uninstall, and rollback

The archive smoke test extracts the exact `.tgz`, runs `--version`, creates and
reads a page in an isolated vault, removes the extracted tree, and confirms its
removal. Native desktop tests install or copy the exact package, verify the
installed application, invoke the platform removal path, and confirm the app
binary is gone.

Those tests use an isolated location or ephemeral runner. An uninstaller must
remove application files only. MyHelp vaults, conflict copies, recovery pages,
configuration, and imported content remain user data.

Updates are explicit downloads, source rebuilds, or changed Nix references for
now. There is no in-app updater. Rollback means reinstalling a previously
published immutable version; MyHelp must continue to read the documented
plaintext format. A future irreversible format migration requires its own
backup and downgrade ADR.

## Consequences

- Users can distinguish portable CLI downloads from native desktop installers.
- Every advertised binary shape has native CI evidence for installation and
  removal.
- The first release assets are consistently named, checksummed, inventoried,
  and attestable.
- Unsigned desktop evaluation remains possible without presenting it as a
  trusted default.
- Store manifests, auto-update, signing credentials, and additional
  architectures remain focused follow-up work rather than untested claims.

## References

- [GitHub artifact attestations](https://docs.github.com/en/actions/concepts/security/artifact-attestations)
- [GitHub immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases)
- [GitHub SBOM REST API and SPDX](https://docs.github.com/en/rest/dependency-graph/sboms)
- [Cargo install sources and lockfiles](https://doc.rust-lang.org/cargo/commands/cargo-install.html)
- [cargo-binstall artifact metadata](https://github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md)
- [Tauri distribution guides](https://v2.tauri.app/distribute/)
- [Tauri updater signatures](https://v2.tauri.app/plugin/updater/#signing-updates)
- [Homebrew formula cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Homebrew cask cookbook](https://docs.brew.sh/Cask-Cookbook)
- [Scoop application manifests](https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests)
- [WinGet package repository](https://learn.microsoft.com/windows/package-manager/package/repository)
- [Flatpak builder](https://docs.flatpak.org/en/latest/flatpak-builder.html)
