# Desktop packaging checks

MyHelp verifies that the desktop app can become a native package before any
package is advertised as a distribution channel. Ordinary pull-request and
`main` CI use the same unsigned smoke matrix:

<!-- markdownlint-disable MD013 MD060 -->

| Runner | Bundle command | Expected package |
|---|---|---|
| Ubuntu | `pnpm tauri build --bundles deb` | `src-tauri/target/release/bundle/deb/*.deb` |
| macOS | `pnpm tauri build --bundles dmg` | `src-tauri/target/release/bundle/dmg/*.dmg` |
| Windows | `pnpm tauri build --bundles nsis` | `src-tauri/target/release/bundle/nsis/*-setup.exe` |

<!-- markdownlint-enable MD013 MD060 -->

The matrix uses native GitHub-hosted runners. Its `fail-fast: false` strategy
keeps one platform failure from cancelling evidence from the other platforms.
Each job first runs the desktop Rust format, test, and clippy checks with the
locked desktop dependency graph. It then installs the frozen pnpm dependency
graph and invokes the repository-pinned Tauri CLI.

Linux installs the packages listed by the official Tauri Debian prerequisite
guide. GitHub's macOS and Windows images provide Xcode Command Line Tools or
the Microsoft C++ build tools and WebView2 required by Tauri.

## Inspection artifacts

A successful job must find the expected package path. CI uploads that package
under an operating-system, architecture, and bundle-specific artifact name
with a seven-day retention period. Missing packages fail the job rather than
producing a warning.

These artifacts are deliberately:

- unsigned and unsuitable for public distribution;
- attached to a workflow run, never silently added to a GitHub Release;
- built without signing certificates, provider credentials, updater keys, or
  repository write permission;
- limited to one representative native package per platform to keep ordinary
  CI proportional.

Artifact presence proves that the native bundler completed on that runner. It
does not prove installation, uninstallation, signing, notarization, reputation,
rollback, or operating-system store acceptance. Those are release-channel
requirements tracked separately in Issue #8.

## Local reproduction

Install the official Tauri prerequisites, run `pnpm install
--frozen-lockfile`, and execute only the command matching the current host:

```bash
# Linux
pnpm tauri build --bundles deb

# macOS
pnpm tauri build --bundles dmg

# Windows
pnpm tauri build --bundles nsis
```

The bundle version comes from `src-tauri/tauri.conf.json`. Keep it aligned with
the root Rust workspace, desktop Rust crate, frontend package, and Nix package.

## References

- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri distribution and bundling](https://v2.tauri.app/distribute/)
- [GitHub Actions artifact retention](https://docs.github.com/en/actions/tutorials/store-and-share-data#configuring-a-custom-artifact-retention-period)
