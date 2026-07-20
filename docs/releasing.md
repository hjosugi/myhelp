# Maintainer release procedure

MyHelp releases one synchronized version for the core, CLI, desktop app,
frontend package, Nix package, and release assets. The automated contract is
defined by
[ADR 0003](adr/0003-release-channels-and-artifact-promotion.md).

## Prepare

1. Start from a clean `main` commit whose ordinary CI is green.
2. Choose the SemVer version and update:
   - root and desktop workspace versions;
   - `package.json`;
   - `src-tauri/tauri.conf.json`;
   - `flake.nix`;
   - internal path dependency versions;
   - `CHANGELOG.md`, including compatibility and distribution impact.
3. Refresh both Rust lockfiles.
4. Run:

```bash
pnpm release:test
pnpm release:check
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
pnpm test
pnpm build
```

Run the desktop Rust and current-host package checks from
[`docs/packaging.md`](packaging.md) as well.

## Publish

Push one annotated tag at the exact reviewed commit:

```bash
git tag -a v0.7.0 -m "MyHelp v0.7.0"
git push origin v0.7.0
```

The workflow rejects lightweight tags, version mismatches, missing dated
changelog entries, and tags whose commit is not reachable from `origin/main`.
Tag CI repeats the native Rust, frontend, brand, Nix, CLI archive, and desktop
bundle checks. It then:

1. rejects version or changelog drift;
2. assembles the exact target matrix;
3. generates the source SBOM and `SHA256SUMS`;
4. creates GitHub provenance attestations;
5. uploads every asset to a draft;
6. downloads and verifies the draft and representative provenance;
7. publishes the draft as a prerelease.

Do not create a second manual release while tag CI is active.

## Verify

After CI succeeds:

```bash
gh release view v0.7.0
gh release download v0.7.0 --dir /tmp/myhelp-v0.7.0
(cd /tmp/myhelp-v0.7.0 && sha256sum --check SHA256SUMS)
gh attestation verify \
  /tmp/myhelp-v0.7.0/myhelp-cli-x86_64-unknown-linux-gnu-v0.7.0.tgz \
  --repo hjosugi/myhelp
```

Confirm the release tag targets the tested commit, the release remains marked
prerelease while MyHelp is pre-1.0, and all expected assets are present.

## Repair or rollback

Never retarget a tag or replace an asset in place. If assembly fails before
publication, inspect and remove only the unpublished draft. Rerun the existing
tag workflow only when the failure was transient and no source or workflow
change is required. If a repair is required, leave the failed tag as evidence,
make the repair on `main`, and issue a new patch version and tag. The workflow
deliberately refuses to overwrite an existing release. If a published release
is bad, mark it clearly, direct users to the last known-good version, and issue
a new patch release.

Desktop auto-update is not enabled. Reinstalling a prior release must not
delete or rewrite the user's plaintext vault.
