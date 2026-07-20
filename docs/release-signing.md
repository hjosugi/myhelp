# Release signing-key threat model

Status: accepted for the pre-1.0 distribution pipeline on 2026-07-19.

This document applies to release publication, CLI archive signatures, native
code signing, notarization, and a future Tauri updater. It complements the
[release-channel ADR](adr/0003-release-channels-and-artifact-promotion.md) and
the [desktop runtime threat model](security.md).

No long-lived signing key is configured for v0.7.0. Its CLI archives use
checksums and keyless GitHub provenance. Its desktop bundles are visibly
unsigned evaluation artifacts, not an advertised desktop channel.

## Protected assets

- The mapping from a MyHelp version and Git commit to exact release bytes.
- Git tags, release notes, checksums, SBOMs, and provenance attestations.
- Apple Developer ID certificates and notarization credentials.
- Windows code-signing identity or signing-provider authorization.
- A future Tauri updater private key and its public key embedded in the app.
- A future cargo-binstall/minisign key and public key in the crate manifest.
- Users' ability to recover from a bad or compromised release.

User vaults are not release credentials. Installers and uninstallers must never
treat a vault as application-owned data.

## Adversaries and failure modes

- An untrusted pull request tries to print, exfiltrate, or use a release secret.
- A compromised dependency, action, build script, or hosted runner changes an
  artifact after tests.
- A maintainer account, token, branch, tag, or GitHub environment is taken over.
- A signing service authorizes the wrong repository, workflow, ref, or actor.
- A private key is copied from logs, caches, artifacts, crash dumps, or a
  maintainer workstation.
- A certificate expires or is revoked, or a private updater key is lost.
- An attacker replaces one release asset while leaving the others untouched.
- A validly signed release contains malicious or destructive application code.
- A release is functionally broken and users need a safe downgrade.

Code signing proves who authorized bytes; it does not prove that the code is
safe. Tests, review, provenance, immutable artifacts, and recovery procedures
remain independent controls.

## Current controls

- Pull-request and ordinary `main` jobs have `contents: read` only.
- Third-party actions are pinned to full commit SHAs.
- The combined pull-request, `main`, and tag workflow does not restore Node or
  Rust build caches, preventing less-trusted runtime state from entering a
  release build.
- Release candidates are built and tested before any publication job receives
  `contents: write`.
- Provenance uses a short-lived GitHub OIDC identity with only
  `id-token: write` and `attestations: write`; no private Sigstore key is stored.
- Publication runs only for an exact `vMAJOR.MINOR.PATCH` tag whose version and
  changelog match the repository.
- CLI and desktop jobs upload separate immutable workflow artifacts. The
  assembly job rejects a missing, extra, nested, or misnamed release file.
- The checksum file is generated from the assembled bytes. Provenance covers
  the release-ready files, including the checksum and SBOM.
- The public release begins as a draft and becomes a prerelease only after the
  complete upload is downloaded again, its asset set and checksums pass, and a
  CLI archive's provenance verifies.
- Existing published tags and assets are never overwritten for a routine
  repair.

GitHub workflow artifacts and OIDC reduce secret exposure but leave GitHub and
the hosted runner in the trusted computing base.

## Credential-specific policy

### GitHub publication and provenance

Use the workflow `GITHUB_TOKEN`, never a personal access token. Keep job
permissions explicit and grant `contents: write` only to the final publication
job. Artifact attestation requires `id-token: write` and
`attestations: write`; those permissions do not belong in build jobs.

Before stable releases, configure a protected `release` environment with
required reviewers and restrict tag creation. The OIDC subject must be scoped
to this repository, the release workflow, and that environment when an
external signing service is added.

### Apple code signing and notarization

Direct-download `.dmg` files require a Developer ID Application identity,
hardened runtime, and notarization before the macOS channel is advertised.
Import a short-lived certificate only inside the protected native macOS job,
use a temporary keychain, and delete it in an always-run cleanup step.

Prefer an App Store Connect API key dedicated to notarization with the smallest
available role. Store the issuer, key identifier, and private key as separate
environment secrets. Never expose them to pull requests, caches, artifacts, or
unsigned build jobs. Record certificate expiry and rehearse renewal before it
becomes urgent.

### Windows code signing

Prefer a managed signing service that accepts GitHub OIDC and restricts the
repository, workflow, environment, tag, and certificate profile. This avoids a
portable long-lived PFX in GitHub Secrets. If a hardware-backed or provider
certificate requires a custom signing command, pin that client and verify the
Authenticode signature after signing.

Timestamp every signed installer so its signature can remain valid after
certificate expiry. Revocation and reputation behavior must be tested on a
clean Windows image before WinGet, Scoop, or the NSIS download is advertised.

### Tauri updater key

The Tauri updater uses a separate application-level signature and does not
allow signature verification to be disabled. Its private key must not be the
Apple, Windows, GitHub, or cargo-binstall key. The public key is embedded in
`tauri.conf.json`; the private key signs updater artifacts only inside the
protected release environment.

Losing this key prevents installed clients from accepting future updates.
Before enabling the plugin:

- create an encrypted offline recovery copy with two documented custodians;
- test restore and signing without using the production release;
- define rotation while the old key can still authorize a transition build;
- define a manual-download recovery path for compromise or unrecoverable loss;
- test a failed download, invalid signature, interrupted installation, and
  rollback to the last format-compatible release.

MyHelp does not enable the updater until these exercises pass. A static update
manifest is release authority and must be protected as carefully as the
artifact.

### cargo-binstall archive key

If cargo-binstall becomes an advertised channel, use a dedicated minisign key
because that is the verifier its metadata currently supports. Commit only the
public key. Sign the final archive after native tests, publish the adjacent
signature, and test installation without `--skip-signatures`.

Keyless GitHub provenance is complementary but is not currently consumed by
cargo-binstall's archive signature check.

## Rotation, revocation, and incident response

Maintain a private inventory containing credential owner, provider, purpose,
creation date, expiry, recovery location, and last restore test. Do not place
private values in that inventory.

On suspected compromise:

1. stop publication and disable the protected release environment;
2. revoke the affected provider credential or certificate;
3. mark affected releases and channels as untrusted without deleting evidence;
4. compare checksums, provenance, workflow logs, and provider audit logs;
5. rotate only through a reviewed recovery build;
6. publish a new patch version and an incident note describing affected
   versions and user action.

A valid signature from a compromised key remains evidence, not reassurance.
Never silently replace the original asset or retarget its tag.

For a non-security functional regression, keep the artifact immutable, document
the last known-good version, publish a patch, and preserve plaintext format
compatibility so users can reinstall the prior version.

## Promotion checklist

An OS-specific desktop or updater channel remains unadvertised until all
applicable items pass:

- signing identity is scoped and independently recoverable;
- build and signing are separate, least-privilege jobs;
- the final signature is verified on a clean native host;
- notarization or timestamping succeeds and is verified;
- exact install, launch, update where applicable, and uninstall paths pass;
- removal leaves vaults and recovery files untouched;
- checksum, SBOM, and provenance correspond to the published bytes;
- bad-release and lost-key recovery have been rehearsed;
- channel documentation states architecture, trust status, and removal steps.

## References

- [GitHub OIDC hardening](https://docs.github.com/en/actions/concepts/security/openid-connect)
- [GitHub artifact attestations](https://docs.github.com/en/actions/concepts/security/artifact-attestations)
- [GitHub environments](https://docs.github.com/en/actions/reference/deployments-and-environments)
- [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/)
- [Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/)
- [Tauri updater signing](https://v2.tauri.app/plugin/updater/#signing-updates)
- [cargo-binstall signature support](https://github.com/cargo-bins/cargo-binstall/blob/main/SIGNING.md)
