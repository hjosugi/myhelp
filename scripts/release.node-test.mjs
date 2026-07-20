import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  CLI_TARGETS,
  cliArchiveName,
  desktopArtifactName,
  expectedReleaseAssets,
  extractChangelogSection,
  normalizeVersion,
  sourceSbomName,
  verifyChecksumManifest,
  writeChecksumManifest,
} from "./release.mjs";

test("normalizes valid release tags and rejects loose versions", () => {
  assert.equal(normalizeVersion("v0.7.0"), "0.7.0");
  assert.equal(normalizeVersion("1.0.0-rc.1"), "1.0.0-rc.1");
  assert.equal(normalizeVersion("1.0.0+build.1"), "1.0.0+build.1");
  assert.throws(() => normalizeVersion("0.7"));
  assert.throws(() => normalizeVersion("1.0.0-01"));
  assert.throws(() => normalizeVersion("release-0.7.0"));
});

test("uses one unambiguous name per CLI and unsigned desktop target", () => {
  assert.deepEqual(
    CLI_TARGETS.map((target) => cliArchiveName("0.7.0", target)),
    [
      "myhelp-cli-x86_64-unknown-linux-gnu-v0.7.0.tgz",
      "myhelp-cli-aarch64-apple-darwin-v0.7.0.tgz",
      "myhelp-cli-x86_64-pc-windows-msvc-v0.7.0.tgz",
    ],
  );
  assert.equal(
    desktopArtifactName("v0.7.0", "aarch64-apple-darwin", "dmg"),
    "myhelp-desktop-aarch64-apple-darwin-v0.7.0-dmg-unsigned.dmg",
  );
  assert.throws(() =>
    desktopArtifactName("0.7.0", "aarch64-apple-darwin", "nsis"),
  );
});

test("extracts only the selected changelog section", () => {
  const changelog = `# Changelog

## [Unreleased]

- Later.

## [0.7.0] - 2026-07-19

### Added

- Distribution contract.

## [0.6.0] - 2026-07-18

- Older.
`;
  assert.equal(
    extractChangelogSection(changelog, "v0.7.0"),
    "### Added\n\n- Distribution contract.",
  );
  assert.throws(() => extractChangelogSection(changelog, "0.8.0"));
});

test("writes sorted SHA-256 checksums only for the complete release set", () => {
  const root = mkdtempSync(join(tmpdir(), "myhelp-release-test-"));
  try {
    for (const name of expectedReleaseAssets("0.7.0").reverse()) {
      writeFileSync(join(root, name), `contents:${name}`, "utf8");
    }

    const manifest = writeChecksumManifest("v0.7.0", root);
    const lines = readFileSync(manifest, "utf8").trimEnd().split("\n");
    const names = lines.map((line) => line.slice(66));
    assert.deepEqual(names, expectedReleaseAssets("0.7.0"));

    const sbom = sourceSbomName("0.7.0");
    const expectedDigest = createHash("sha256")
      .update(`contents:${sbom}`)
      .digest("hex");
    assert.ok(lines.includes(`${expectedDigest}  ${sbom}`));
    verifyChecksumManifest("v0.7.0", root);

    writeFileSync(join(root, sbom), "tampered", "utf8");
    assert.throws(
      () => verifyChecksumManifest("0.7.0", root),
      /SHA-256 mismatch/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects incomplete and nested release-ready directories", () => {
  const incomplete = mkdtempSync(join(tmpdir(), "myhelp-release-test-"));
  const nested = mkdtempSync(join(tmpdir(), "myhelp-release-test-"));
  try {
    assert.throws(() => writeChecksumManifest("0.7.0", incomplete));
    for (const name of expectedReleaseAssets("0.7.0")) {
      writeFileSync(join(nested, name), name, "utf8");
    }
    mkdirSync(join(nested, "unexpected"));
    assert.throws(() => writeChecksumManifest("0.7.0", nested));
  } finally {
    rmSync(incomplete, { recursive: true, force: true });
    rmSync(nested, { recursive: true, force: true });
  }
});

test("rejects extra published assets and malformed checksum paths", () => {
  const root = mkdtempSync(join(tmpdir(), "myhelp-release-test-"));
  try {
    for (const name of expectedReleaseAssets("0.7.0")) {
      writeFileSync(join(root, name), name, "utf8");
    }
    writeChecksumManifest("0.7.0", root);
    writeFileSync(join(root, "unexpected.txt"), "unexpected", "utf8");
    assert.throws(
      () => verifyChecksumManifest("0.7.0", root),
      /published assets differ/,
    );

    rmSync(join(root, "unexpected.txt"));
    const manifest = join(root, "SHA256SUMS");
    const content = readFileSync(manifest, "utf8");
    writeFileSync(manifest, content.replace("  myhelp-", "  ../myhelp-"), "utf8");
    assert.throws(
      () => verifyChecksumManifest("0.7.0", root),
      /invalid SHA256SUMS line/,
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
