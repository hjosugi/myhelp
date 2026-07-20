import {
  chmodSync,
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";
import assert from "node:assert/strict";

export const CLI_TARGETS = Object.freeze([
  "x86_64-unknown-linux-gnu",
  "aarch64-apple-darwin",
  "x86_64-pc-windows-msvc",
]);

export const DESKTOP_TARGETS = Object.freeze([
  Object.freeze({
    target: "x86_64-unknown-linux-gnu",
    bundle: "deb",
    sourceSuffix: ".deb",
    extension: "deb",
  }),
  Object.freeze({
    target: "aarch64-apple-darwin",
    bundle: "dmg",
    sourceSuffix: ".dmg",
    extension: "dmg",
  }),
  Object.freeze({
    target: "x86_64-pc-windows-msvc",
    bundle: "nsis",
    sourceSuffix: "-setup.exe",
    extension: "exe",
  }),
]);

const SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

function fail(message) {
  throw new Error(message);
}

export function normalizeVersion(value) {
  if (typeof value !== "string") {
    fail(`expected a SemVer version, received ${JSON.stringify(value)}`);
  }
  const version = value.startsWith("v") ? value.slice(1) : value;
  if (!SEMVER.test(version)) {
    fail(`expected a SemVer version, received ${JSON.stringify(value)}`);
  }
  return version;
}

function requireSupported(value, supported, label) {
  if (!supported.includes(value)) {
    fail(`unsupported ${label} ${JSON.stringify(value)}`);
  }
}

export function cliArchiveName(versionValue, target) {
  const version = normalizeVersion(versionValue);
  requireSupported(target, CLI_TARGETS, "CLI target");
  return `myhelp-cli-${target}-v${version}.tgz`;
}

export function desktopArtifactName(versionValue, target, bundle) {
  const version = normalizeVersion(versionValue);
  const config = DESKTOP_TARGETS.find(
    (entry) => entry.target === target && entry.bundle === bundle,
  );
  if (!config) {
    fail(
      `unsupported desktop target/bundle ${JSON.stringify(`${target}/${bundle}`)}`,
    );
  }
  return `myhelp-desktop-${target}-v${version}-${bundle}-unsigned.${config.extension}`;
}

export function sourceSbomName(versionValue) {
  const version = normalizeVersion(versionValue);
  return `myhelp-source-v${version}.spdx.json`;
}

export function expectedReleaseAssets(versionValue) {
  const version = normalizeVersion(versionValue);
  return [
    ...CLI_TARGETS.map((target) => cliArchiveName(version, target)),
    ...DESKTOP_TARGETS.map(({ target, bundle }) =>
      desktopArtifactName(version, target, bundle),
    ),
    sourceSbomName(version),
  ].sort();
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    ...options,
  });
  if (result.error && result.status === null) {
    throw result.error;
  }
  if (result.status !== 0) {
    const output = [result.stdout, result.stderr].filter(Boolean).join("\n");
    fail(
      `${command} ${args.join(" ")} failed with exit code ${result.status}${
        output ? `\n${output.trim()}` : ""
      }`,
    );
  }
  return result;
}

function executableName(target) {
  return target.endsWith("-windows-msvc") ? "myhelp.exe" : "myhelp";
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function stageCliArchive({
  version: versionValue,
  target,
  binary,
  outputDirectory,
}) {
  const version = normalizeVersion(versionValue);
  requireSupported(target, CLI_TARGETS, "CLI target");

  const source = resolve(binary);
  if (
    !existsSync(source) ||
    lstatSync(source).isSymbolicLink() ||
    !statSync(source).isFile()
  ) {
    fail(`CLI binary does not exist: ${source}`);
  }

  const output = resolve(outputDirectory);
  mkdirSync(output, { recursive: true });
  const archiveName = cliArchiveName(version, target);
  const rootName = archiveName.slice(0, -".tgz".length);
  const root = join(output, rootName);
  const archive = join(output, archiveName);
  rmSync(root, { recursive: true, force: true });
  rmSync(archive, { force: true });
  mkdirSync(root);

  const packagedBinary = join(root, executableName(target));
  copyFileSync(source, packagedBinary);
  if (!target.endsWith("-windows-msvc")) {
    chmodSync(packagedBinary, 0o755);
  }
  copyFileSync(resolve("LICENSE"), join(root, "LICENSE"));
  copyFileSync(resolve("README.md"), join(root, "README.md"));

  try {
    run("tar", ["-czf", archiveName, rootName], {
      cwd: output,
      stdio: "pipe",
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }

  if (!existsSync(archive) || statSync(archive).size === 0) {
    fail(`archive was not created: ${archive}`);
  }
  return archive;
}

function walkRegularFiles(directory) {
  const root = resolve(directory);
  const files = [];

  function walk(current) {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const path = join(current, entry.name);
      if (entry.isSymbolicLink()) {
        fail(`release input must not contain a symbolic link: ${path}`);
      }
      if (entry.isDirectory()) {
        walk(path);
      } else if (entry.isFile()) {
        files.push(path);
      } else {
        fail(`release input must contain regular files only: ${path}`);
      }
    }
  }

  walk(root);
  return files.sort();
}

export function stageDesktopArtifact({
  version: versionValue,
  target,
  bundle,
  inputDirectory,
  outputDirectory,
}) {
  const version = normalizeVersion(versionValue);
  const config = DESKTOP_TARGETS.find(
    (entry) => entry.target === target && entry.bundle === bundle,
  );
  if (!config) {
    fail(
      `unsupported desktop target/bundle ${JSON.stringify(`${target}/${bundle}`)}`,
    );
  }

  const matches = walkRegularFiles(inputDirectory).filter((path) =>
    basename(path).endsWith(config.sourceSuffix),
  );
  if (matches.length !== 1) {
    fail(
      `expected exactly one ${config.sourceSuffix} bundle under ${resolve(
        inputDirectory,
      )}, found ${matches.length}`,
    );
  }

  const output = resolve(outputDirectory);
  mkdirSync(output, { recursive: true });
  const destination = join(
    output,
    desktopArtifactName(version, target, bundle),
  );
  copyFileSync(matches[0], destination);
  return destination;
}

function smokeCommand(binary, pagesDirectory, args) {
  return run(binary, ["--pages-dir", pagesDirectory, ...args], {
    stdio: "pipe",
    env: {
      ...process.env,
      NO_COLOR: "1",
    },
  });
}

export function smokeCliArchive(archiveValue, target) {
  requireSupported(target, CLI_TARGETS, "CLI target");
  const archive = resolve(archiveValue);
  if (!existsSync(archive) || !statSync(archive).isFile()) {
    fail(`CLI archive does not exist: ${archive}`);
  }

  const expectedNamePattern = new RegExp(
    `^myhelp-cli-${escapeRegExp(target)}-v(${SEMVER.source.slice(
      1,
      -1,
    )})\\.tgz$`,
  );
  const match = basename(archive).match(expectedNamePattern);
  if (!match) {
    fail(`archive name does not match the release contract: ${basename(archive)}`);
  }
  const version = normalizeVersion(match[1]);
  const extraction = mkdtempSync(join(tmpdir(), "myhelp-release-smoke-"));
  const installRoot = join(
    extraction,
    cliArchiveName(version, target).slice(0, -".tgz".length),
  );
  const pages = join(extraction, "vault");
  const binary = join(installRoot, executableName(target));
  const archiveRoot = basename(installRoot);
  const archiveDirectory = dirname(archive);
  const archiveName = basename(archive);
  const expectedEntries = [
    `${archiveRoot}/`,
    `${archiveRoot}/LICENSE`,
    `${archiveRoot}/README.md`,
    `${archiveRoot}/${executableName(target)}`,
  ].sort();

  try {
    const entries = run("tar", ["-tzf", archiveName], {
      cwd: archiveDirectory,
      stdio: "pipe",
    }).stdout
      .split(/\r?\n/)
      .filter(Boolean)
      .sort();
    assert.deepEqual(
      entries,
      expectedEntries,
      "CLI archive must contain only the documented target/version directory",
    );

    run("tar", ["-xzf", archiveName, "-C", extraction], {
      cwd: archiveDirectory,
      stdio: "pipe",
    });
    const extractedFiles = walkRegularFiles(installRoot)
      .map((path) => basename(path))
      .sort();
    assert.deepEqual(extractedFiles, [
      "LICENSE",
      "README.md",
      executableName(target),
    ].sort());
    if (!existsSync(binary) || !statSync(binary).isFile()) {
      fail(`archive does not contain the expected binary: ${binary}`);
    }
    if (!target.endsWith("-windows-msvc")) {
      chmodSync(binary, 0o755);
    }

    const versionOutput = run(binary, ["--version"], {
      stdio: "pipe",
    }).stdout.trim();
    assert.match(versionOutput, new RegExp(`\\b${escapeRegExp(version)}$`));

    smokeCommand(binary, pages, [
      "new",
      "release-smoke",
      "--title",
      "Release smoke",
    ]);
    const shown = smokeCommand(binary, pages, [
      "show",
      "release-smoke",
      "--raw",
    ]).stdout;
    assert.match(shown, /^# Release smoke/m);
  } finally {
    rmSync(extraction, { recursive: true, force: true });
  }

  assert.equal(
    existsSync(extraction),
    false,
    "archive uninstall must remove the extracted tree",
  );
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

export function writeChecksumManifest(versionValue, directoryValue) {
  const version = normalizeVersion(versionValue);
  const directory = resolve(directoryValue);
  const expected = expectedReleaseAssets(version);
  const entries = readdirSync(directory, { withFileTypes: true });
  const actual = entries.map((entry) => {
    if (!entry.isFile()) {
      fail(`release-ready directory must be flat: ${entry.name}`);
    }
    return entry.name;
  });
  actual.sort();
  assert.deepEqual(
    actual,
    expected,
    `release-ready assets differ from the v${version} contract`,
  );

  const lines = actual.map(
    (name) => `${sha256(join(directory, name))}  ${name}`,
  );
  const manifest = join(directory, "SHA256SUMS");
  writeFileSync(manifest, `${lines.join("\n")}\n`, "utf8");
  return manifest;
}

function listFlatRegularFiles(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .map((entry) => {
      if (!entry.isFile() || entry.isSymbolicLink()) {
        fail(`release-ready directory must contain regular files only: ${entry.name}`);
      }
      return entry.name;
    })
    .sort();
}

export function verifyChecksumManifest(versionValue, directoryValue) {
  const version = normalizeVersion(versionValue);
  const directory = resolve(directoryValue);
  const assets = expectedReleaseAssets(version);
  assert.deepEqual(
    listFlatRegularFiles(directory),
    [...assets, "SHA256SUMS"].sort(),
    `published assets differ from the v${version} contract`,
  );

  const manifestPath = join(directory, "SHA256SUMS");
  const manifest = readFileSync(manifestPath, "utf8");
  if (!manifest.endsWith("\n")) {
    fail("SHA256SUMS must end with a newline");
  }
  const records = manifest
    .slice(0, -1)
    .split("\n")
    .map((line) => {
      const match = line.match(/^([0-9a-f]{64})  ([^/\\]+)$/);
      if (!match) {
        fail(`invalid SHA256SUMS line: ${JSON.stringify(line)}`);
      }
      return { digest: match[1], name: match[2] };
    });
  assert.deepEqual(
    records.map(({ name }) => name),
    assets,
    "SHA256SUMS must list every release asset once in sorted order",
  );
  for (const { digest, name } of records) {
    assert.equal(
      sha256(join(directory, name)),
      digest,
      `SHA-256 mismatch for ${name}`,
    );
  }
}

function tomlSection(text, name) {
  const heading = `[${name}]`;
  const start = text.indexOf(heading);
  if (start === -1) {
    fail(`missing TOML section ${heading}`);
  }
  const bodyStart = start + heading.length;
  const next = text.slice(bodyStart).search(/^\[/m);
  return next === -1
    ? text.slice(bodyStart)
    : text.slice(bodyStart, bodyStart + next);
}

function tomlVersion(path, section) {
  const body = tomlSection(readFileSync(path, "utf8"), section);
  const match = body.match(/^\s*version\s*=\s*"([^"]+)"\s*$/m);
  if (!match) {
    fail(`missing version in ${path} [${section}]`);
  }
  return match[1];
}

function inlineDependencyVersion(path, section, dependency) {
  const body = tomlSection(readFileSync(path, "utf8"), section);
  const escapedDependency = escapeRegExp(dependency);
  const match = body.match(
    new RegExp(
      `^\\s*${escapedDependency}\\s*=\\s*\\{[^\\n]*\\bversion\\s*=\\s*"([^"]+)"[^\\n]*\\}\\s*$`,
      "m",
    ),
  );
  if (!match) {
    fail(`missing versioned ${dependency} dependency in ${path} [${section}]`);
  }
  return match[1];
}

function lockPackageVersions(path, names) {
  const blocks = readFileSync(path, "utf8").split("[[package]]").slice(1);
  const versions = new Map();
  for (const block of blocks) {
    const name = block.match(/^\s*name\s*=\s*"([^"]+)"\s*$/m)?.[1];
    const version = block.match(/^\s*version\s*=\s*"([^"]+)"\s*$/m)?.[1];
    if (name && version && names.includes(name)) {
      versions.set(name, version);
    }
  }
  for (const name of names) {
    if (!versions.has(name)) {
      fail(`missing ${name} package in ${path}`);
    }
  }
  return versions;
}

function changelogHeading(content, version) {
  return new RegExp(
    `^## \\[${escapeRegExp(version)}\\] - \\d{4}-\\d{2}-\\d{2}$`,
    "m",
  );
}

export function extractChangelogSection(content, versionValue) {
  const version = normalizeVersion(versionValue);
  const heading = changelogHeading(content, version);
  const match = heading.exec(content);
  if (!match) {
    fail(`CHANGELOG.md has no dated v${version} section`);
  }
  const start = match.index + match[0].length;
  const remainder = content.slice(start);
  const nextHeading = remainder.search(/^## \[/m);
  const body = (nextHeading === -1 ? remainder : remainder.slice(0, nextHeading))
    .trim();
  if (!body) {
    fail(`CHANGELOG.md v${version} section is empty`);
  }
  return body;
}

export function validateRepositoryVersion(tagValue) {
  const packageJson = JSON.parse(readFileSync("package.json", "utf8"));
  const tauriConfig = JSON.parse(
    readFileSync("src-tauri/tauri.conf.json", "utf8"),
  );
  const expected = normalizeVersion(packageJson.version);
  const versions = new Map([
    ["package.json", packageJson.version],
    ["Cargo.toml", tomlVersion("Cargo.toml", "workspace.package")],
    [
      "src-tauri/Cargo.toml",
      tomlVersion("src-tauri/Cargo.toml", "workspace.package"),
    ],
    ["src-tauri/tauri.conf.json", tauriConfig.version],
    [
      "crates/myhelp-cli/Cargo.toml:myhelp-core",
      inlineDependencyVersion(
        "crates/myhelp-cli/Cargo.toml",
        "dependencies",
        "myhelp-core",
      ),
    ],
    [
      "src-tauri/Cargo.toml:myhelp-core",
      inlineDependencyVersion(
        "src-tauri/Cargo.toml",
        "dependencies",
        "myhelp-core",
      ),
    ],
  ]);

  const flakeVersion = readFileSync("flake.nix", "utf8").match(
    /pname\s*=\s*"myhelp";\s*version\s*=\s*"([^"]+)";/s,
  )?.[1];
  if (!flakeVersion) {
    fail("missing MyHelp package version in flake.nix");
  }
  versions.set("flake.nix", flakeVersion);

  for (const [name, version] of lockPackageVersions("Cargo.lock", [
    "myhelp-cli",
    "myhelp-core",
  ])) {
    versions.set(`Cargo.lock:${name}`, version);
  }
  for (const [name, version] of lockPackageVersions("src-tauri/Cargo.lock", [
    "myhelp-core",
    "myhelp-desktop",
  ])) {
    versions.set(`src-tauri/Cargo.lock:${name}`, version);
  }

  const mismatches = [...versions].filter(([, version]) => version !== expected);
  if (mismatches.length > 0) {
    fail(
      `release versions must all equal ${expected}: ${mismatches
        .map(([name, version]) => `${name}=${version}`)
        .join(", ")}`,
    );
  }

  if (tagValue && normalizeVersion(tagValue) !== expected) {
    fail(`release tag ${tagValue} does not match repository version ${expected}`);
  }

  extractChangelogSection(readFileSync("CHANGELOG.md", "utf8"), expected);
  return expected;
}

function writeReleaseNotes(versionValue, outputValue) {
  const version = normalizeVersion(versionValue);
  const body = extractChangelogSection(
    readFileSync("CHANGELOG.md", "utf8"),
    version,
  );
  writeFileSync(resolve(outputValue), `${body}\n`, "utf8");
}

function usage() {
  return [
    "Usage:",
    "  node scripts/release.mjs validate [vVERSION]",
    "  node scripts/release.mjs stage-cli VERSION TARGET BINARY OUTPUT_DIR",
    "  node scripts/release.mjs smoke-cli ARCHIVE TARGET",
    "  node scripts/release.mjs stage-desktop VERSION TARGET BUNDLE INPUT_DIR OUTPUT_DIR",
    "  node scripts/release.mjs manifest VERSION DIRECTORY",
    "  node scripts/release.mjs verify VERSION DIRECTORY",
    "  node scripts/release.mjs notes VERSION OUTPUT_FILE",
  ].join("\n");
}

function main(args) {
  const [command, ...rest] = args;
  switch (command) {
    case "validate": {
      if (rest.length > 1) fail(usage());
      process.stdout.write(`${validateRepositoryVersion(rest[0])}\n`);
      return;
    }
    case "stage-cli": {
      if (rest.length !== 4) fail(usage());
      const [version, target, binary, outputDirectory] = rest;
      process.stdout.write(
        `${stageCliArchive({ version, target, binary, outputDirectory })}\n`,
      );
      return;
    }
    case "smoke-cli": {
      if (rest.length !== 2) fail(usage());
      smokeCliArchive(rest[0], rest[1]);
      return;
    }
    case "stage-desktop": {
      if (rest.length !== 5) fail(usage());
      const [version, target, bundle, inputDirectory, outputDirectory] = rest;
      process.stdout.write(
        `${stageDesktopArtifact({
          version,
          target,
          bundle,
          inputDirectory,
          outputDirectory,
        })}\n`,
      );
      return;
    }
    case "manifest": {
      if (rest.length !== 2) fail(usage());
      process.stdout.write(`${writeChecksumManifest(rest[0], rest[1])}\n`);
      return;
    }
    case "verify": {
      if (rest.length !== 2) fail(usage());
      verifyChecksumManifest(rest[0], rest[1]);
      return;
    }
    case "notes": {
      if (rest.length !== 2) fail(usage());
      writeReleaseNotes(rest[0], rest[1]);
      return;
    }
    default:
      fail(usage());
  }
}

const isMain =
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href;

if (isMain) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(
      `release contract error: ${
        error instanceof Error ? error.message : String(error)
      }\n`,
    );
    process.exitCode = 1;
  }
}
