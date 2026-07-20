# Packaging and tested installation paths

MyHelp treats package creation, package testing, and channel promotion as
different states. The governing decision is
[ADR 0003](adr/0003-release-channels-and-artifact-promotion.md); credentials
and recovery are covered by the
[release signing threat model](release-signing.md).

## Release matrix

Ordinary pull-request, `main`, and release-tag CI all build the same native
matrix:

<!-- markdownlint-disable MD013 MD060 -->

| Product | Runner | Build command | Tested output |
|---|---|---|---|
| CLI | Ubuntu x64 | `cargo build -p myhelp-cli --release --locked` | `myhelp-cli-x86_64-unknown-linux-gnu-vVERSION.tgz` |
| CLI | macOS ARM64 | `cargo build -p myhelp-cli --release --locked` | `myhelp-cli-aarch64-apple-darwin-vVERSION.tgz` |
| CLI | Windows x64 | `cargo build -p myhelp-cli --release --locked` | `myhelp-cli-x86_64-pc-windows-msvc-vVERSION.tgz` |
| Desktop | Ubuntu x64 | `pnpm tauri build --bundles deb` | `myhelp-desktop-x86_64-unknown-linux-gnu-vVERSION-deb-unsigned.deb` |
| Desktop | macOS ARM64 | `pnpm tauri build --bundles dmg` | `myhelp-desktop-aarch64-apple-darwin-vVERSION-dmg-unsigned.dmg` |
| Desktop | Windows x64 | `pnpm tauri build --bundles nsis` | `myhelp-desktop-x86_64-pc-windows-msvc-vVERSION-nsis-unsigned.exe` |

<!-- markdownlint-enable MD013 MD060 -->

The CLI and desktop jobs upload separate seven-day workflow artifacts. Missing,
extra, nested, or differently named files fail the assembly job.

## What the smoke tests prove

Each CLI job packages the release binary, MIT license, and README under one
target/version directory. It then extracts that exact `.tgz` into an isolated
location, checks `myhelp --version`, creates and reads a page in an isolated
vault, removes the extracted tree, and confirms removal.

The same native matrix also runs a locked `cargo install --path` equivalent,
checks the installed CLI, uninstalls the crate, and confirms the binary is
gone.

The native desktop jobs test the exact package before normalizing its name:

- Ubuntu installs the `deb` with `dpkg`, checks the registered package and
  `/usr/bin/myhelp-desktop`, removes `my-help`, and checks that the executable
  is gone.
- macOS mounts the read-only DMG, copies `MyHelp.app` to an isolated
  Applications directory, checks the app's native executable, removes the app,
  and detaches the image.
- Windows silently installs the NSIS package to an isolated directory, checks
  the app executable and uninstaller, runs the silent uninstaller, and checks
  that application files are gone.

The Nix job runs `nix flake check --no-update-lock-file` and the CLI on native
Linux and macOS hosts. The Rust matrix continues to test the locked workspace
on Linux, macOS, and Windows.

These checks prove a clean package path on a disposable host. They do not prove
operating-system publisher trust, notarization, SmartScreen reputation, store
acceptance, or in-app rollback.

## Release assembly

Every pull request, `main` build, and release tag downloads the six native
candidates into one flat staging directory, generates an SPDX JSON source SBOM
with pinned Syft v1.48.0, and requires the exact release contract defined by
`scripts/release.mjs`. It emits `SHA256SUMS` in sorted filename order and
uploads the assembled candidate for seven days.

For a release tag only, GitHub creates keyless provenance for the staged
artifacts. The final publication job receives repository write permission only
after Rust, desktop, frontend, brand, Nix, archive, and assembly jobs succeed.
It uploads the complete set to a draft, downloads and rechecks the draft,
verifies representative provenance, and only then publishes a prerelease.

The SBOM inventories the checked-out source dependency graph. Native frameworks
that Syft cannot discover are a documented residual gap; this is not a
per-installer completeness claim.

## Advertised CLI archives

Download the archive and `SHA256SUMS` for the current target from the matching
GitHub prerelease.

On Linux, verify and install the x64 archive from its download directory:

```bash
sha256sum --check SHA256SUMS --ignore-missing
tar -xzf myhelp-cli-x86_64-unknown-linux-gnu-v0.7.0.tgz
mkdir -p "$HOME/.local/bin"
install -m 0755 \
  myhelp-cli-x86_64-unknown-linux-gnu-v0.7.0/myhelp \
  "$HOME/.local/bin/myhelp"
myhelp --version
```

On macOS Apple Silicon, use the platform checksum command and the corresponding
archive:

```bash
archive=myhelp-cli-aarch64-apple-darwin-v0.7.0.tgz
grep -F "  $archive" SHA256SUMS | shasum -a 256 --check
tar -xzf "$archive"
mkdir -p "$HOME/.local/bin"
install -m 0755 \
  myhelp-cli-aarch64-apple-darwin-v0.7.0/myhelp \
  "$HOME/.local/bin/myhelp"
myhelp --version
```

On Windows x64, PowerShell can verify and extract the `.tgz` without a
third-party archive tool:

```powershell
$archive = "myhelp-cli-x86_64-pc-windows-msvc-v0.7.0.tgz"
$expected = (
  Select-String -Path SHA256SUMS -Pattern "  $([regex]::Escape($archive))$"
).Line.Split()[0]
$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "SHA-256 mismatch for $archive" }
tar.exe -xzf $archive
New-Item -ItemType Directory -Force "$HOME\bin" | Out-Null
Copy-Item `
  "myhelp-cli-x86_64-pc-windows-msvc-v0.7.0\myhelp.exe" `
  "$HOME\bin\myhelp.exe"
& "$HOME\bin\myhelp.exe" --version
```

Add the selected destination directory to `PATH` if it is not already there.
Update any archive installation by verifying a newer immutable version and
replacing that one binary. Uninstall without touching the vault:

```bash
rm "$HOME/.local/bin/myhelp"
```

```powershell
Remove-Item "$HOME\bin\myhelp.exe"
```

Provenance is an additional check:

```bash
gh attestation verify \
  myhelp-cli-x86_64-unknown-linux-gnu-v0.7.0.tgz \
  --repo hjosugi/myhelp
```

## Source-build alternatives

Cargo can build the CLI from the immutable release tag:

```bash
cargo install \
  --git https://github.com/hjosugi/myhelp \
  --tag v0.7.0 \
  --locked \
  myhelp-cli
myhelp --version
cargo uninstall myhelp-cli
```

The manifest also describes the tested archive layout for cargo-binstall.
That channel is not advertised yet: MyHelp must first publish the discoverable
crate metadata and test minisign verification without a signature bypass.

The Nix Flake remains a no-persistent-install path:

```bash
nix run github:hjosugi/myhelp/v0.7.0 -- --version
nix run github:hjosugi/myhelp/v0.7.0 -- list
```

Change the versioned reference to update. `nix run` does not place a MyHelp
binary in a user profile; ordinary Nix garbage collection manages its store
closure.

## Desktop status

The `deb`, `dmg`, and NSIS files are attached to prereleases only as evaluation
artifacts. Their filename says `unsigned`, and the README does not present them
as a trusted desktop installation channel. Do not bypass a platform security
warning on a machine that holds important data.

Promotion requires the platform code-signing, recovery, and clean-host checks
in `docs/release-signing.md`. MyHelp does not ship the Tauri updater plugin;
updates remain explicit until mandatory updater signing and rollback exercises
pass.

Removing the desktop application must never remove a MyHelp vault, conflict
copy, recoverable deletion, imported page, or user-selected configuration.

## Local reproduction

Install the official Tauri prerequisites and frozen frontend dependencies, then
run only the native package command for the current host:

```bash
pnpm install --frozen-lockfile
pnpm release:test
pnpm release:check

# Linux
pnpm tauri build --bundles deb

# macOS
pnpm tauri build --bundles dmg

# Windows
pnpm tauri build --bundles nsis
```

Create and exercise the current CLI archive using the same contract script:

```bash
nix develop --command cargo build -p myhelp-cli --release --locked
node scripts/release.mjs stage-cli \
  0.7.0 \
  x86_64-unknown-linux-gnu \
  target/release/myhelp \
  release-assets
node scripts/release.mjs smoke-cli \
  release-assets/myhelp-cli-x86_64-unknown-linux-gnu-v0.7.0.tgz \
  x86_64-unknown-linux-gnu
```

## References

- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri distribution and bundling](https://v2.tauri.app/distribute/)
- [GitHub artifact attestations](https://docs.github.com/en/actions/concepts/security/artifact-attestations)
- [SPDX](https://spdx.dev/)
- [Syft](https://github.com/anchore/syft)
