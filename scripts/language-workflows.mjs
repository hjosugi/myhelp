import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const PAGE_SUFFIX = ".page.md";
const DEFAULT_PACK_DIRECTORY = resolve("examples/language-workflows");
const UPDATE_COMMAND =
  /(?:\bmix deps\.update\b|\bgleam update\b|\bgo get -u\b|\buv lock --upgrade\b|\bcargo update\b|\bpnpm update\b)/;
const DESTRUCTIVE_COMMAND =
  /(?:^|(?:&&|\|\||;|\|)\s*)(?:rm|rmdir|del)\s|\b(?:remove-item)\b/i;
const MAJOR_UPDATE_COMMAND = /(?:--major\b|--latest\b|@latest\b)/i;

function readUtf8(path) {
  const bytes = readFileSync(path);
  return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

function formatErrors(errors) {
  return new Error(
    `language workflow validation failed:\n${errors
      .map((error) => `- ${error}`)
      .join("\n")}`,
  );
}

function arraysEqual(left, right) {
  return (
    left.length === right.length &&
    left.every((value, index) => value === right[index])
  );
}

function commandExamples(lines) {
  const examples = [];
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (!/^`[^`\r\n]+`$/.test(line)) continue;

    let description = "";
    for (let previous = index - 1; previous >= 0; previous -= 1) {
      if (lines[previous] === "") continue;
      description = lines[previous].startsWith("- ")
        ? lines[previous].slice(2)
        : "";
      break;
    }
    examples.push({ command: line.slice(1, -1), description, line: index + 1 });
  }
  return examples;
}

export function validateLanguageWorkflowPack(
  directoryValue = DEFAULT_PACK_DIRECTORY,
) {
  const directory = resolve(directoryValue);
  const manifestPath = join(directory, "manifest.json");
  const errors = [];
  let manifest;

  try {
    manifest = JSON.parse(readUtf8(manifestPath));
  } catch (error) {
    throw formatErrors([
      `${basename(manifestPath)} is not valid UTF-8 JSON: ${
        error instanceof Error ? error.message : String(error)
      }`,
    ]);
  }

  if (manifest?.schemaVersion !== 1) {
    errors.push("manifest.json schemaVersion must be 1");
  }
  if (!/^\d{4}-\d{2}-\d{2}$/.test(manifest?.reviewedOn ?? "")) {
    errors.push("manifest.json reviewedOn must be an ISO date");
  }
  if (!Array.isArray(manifest?.pages) || manifest.pages.length === 0) {
    errors.push("manifest.json pages must be a non-empty array");
  }

  const pageFiles = readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(PAGE_SUFFIX))
    .map((entry) => entry.name)
    .sort();
  const entries = Array.isArray(manifest?.pages) ? manifest.pages : [];
  const manifestFiles = entries.map((entry) => entry?.file);
  const sortedManifestFiles = [...manifestFiles].sort();

  if (!arraysEqual(manifestFiles, sortedManifestFiles)) {
    errors.push("manifest.json pages must be sorted by file");
  }
  if (!arraysEqual(pageFiles, sortedManifestFiles)) {
    errors.push(
      `manifest/page files differ (manifest: ${sortedManifestFiles.join(", ")}; files: ${pageFiles.join(", ")})`,
    );
  }
  if (new Set(manifestFiles).size !== manifestFiles.length) {
    errors.push("manifest.json contains duplicate page files");
  }

  const topics = [];
  let documentationLinkCount = 0;
  for (const entry of entries) {
    const file = entry?.file;
    const topic = entry?.topic;
    if (typeof file !== "string" || !file.endsWith(PAGE_SUFFIX)) {
      errors.push(`manifest entry has an invalid file: ${JSON.stringify(file)}`);
      continue;
    }
    if (typeof topic !== "string" || file !== `${topic}${PAGE_SUFFIX}`) {
      errors.push(`${file} must use its filename stem as topic`);
    } else {
      topics.push(topic);
    }
    if (typeof entry?.toolchain !== "string" || entry.toolchain.trim() === "") {
      errors.push(`${file} must declare a toolchain assumption`);
    }
    if (
      !Array.isArray(entry?.documentation) ||
      entry.documentation.length === 0 ||
      entry.documentation.some((url) => typeof url !== "string")
    ) {
      errors.push(`${file} must declare official documentation URLs`);
      continue;
    }

    const path = join(directory, file);
    try {
      const status = lstatSync(path);
      if (status.isSymbolicLink() || !status.isFile()) {
        errors.push(`${file} must be a regular file, not a symlink`);
        continue;
      }
    } catch (error) {
      errors.push(
        `${file} cannot be read: ${error instanceof Error ? error.message : String(error)}`,
      );
      continue;
    }

    let content;
    try {
      content = readUtf8(path);
    } catch (error) {
      errors.push(
        `${file} is not valid UTF-8: ${error instanceof Error ? error.message : String(error)}`,
      );
      continue;
    }
    const lines = content.split(/\r?\n/);
    if (!content.endsWith("\n")) errors.push(`${file} must end with a newline`);
    if (!/^# \S/.test(lines[0] ?? "")) {
      errors.push(`${file}:1 must be a level-one title`);
    }
    if (!(lines[2] ?? "").startsWith("> ")) {
      errors.push(`${file}:3 must begin the page description`);
    }

    const documentationLines = lines.filter((line) =>
      line.startsWith("> Official documentation:"),
    );
    if (documentationLines.length !== 1) {
      errors.push(`${file} must contain exactly one official documentation line`);
    }
    const links = [
      ...(documentationLines[0] ?? "").matchAll(/<([^<>]+)>/g),
    ].map((match) => match[1]);
    if (!arraysEqual(links, entry.documentation)) {
      errors.push(`${file} documentation URLs must match manifest.json`);
    }
    for (const link of links) {
      documentationLinkCount += 1;
      try {
        const url = new URL(link);
        if (url.protocol !== "https:" || url.username || url.password) {
          errors.push(`${file} has a non-public HTTPS documentation URL: ${link}`);
        }
      } catch {
        errors.push(`${file} has an invalid documentation URL: ${link}`);
      }
    }

    if (
      /(?:^|[\s`"'])\/(?:home|Users)\/[^\s/`"']+/m.test(content) ||
      /\b[A-Za-z]:\\Users\\/i.test(content)
    ) {
      errors.push(`${file} contains a machine-specific home path`);
    }

    for (const example of commandExamples(lines)) {
      const location = `${file}:${example.line}`;
      if (UPDATE_COMMAND.test(example.command) && !/\breview\b/i.test(example.description)) {
        errors.push(`${location} dependency update must tell the reader to review changes`);
      }
      if (
        DESTRUCTIVE_COMMAND.test(example.command) &&
        !/\bdestructive\b/i.test(example.description)
      ) {
        errors.push(`${location} destructive command must be labeled Destructive`);
      }
      if (
        MAJOR_UPDATE_COMMAND.test(example.command) &&
        !/\bmajor update\b/i.test(example.description)
      ) {
        errors.push(`${location} unbounded update must be labeled Major update`);
      }
    }
  }

  if (new Set(topics).size !== topics.length) {
    errors.push("manifest.json contains duplicate topics");
  }
  if (errors.length > 0) throw formatErrors(errors);

  return { pageCount: entries.length, documentationLinkCount };
}

function main(args) {
  if (args.length > 1) {
    throw new Error("Usage: node scripts/language-workflows.mjs [PACK_DIRECTORY]");
  }
  const report = validateLanguageWorkflowPack(args[0]);
  process.stdout.write(
    `Validated ${report.pageCount} language workflow pages and ${report.documentationLinkCount} official documentation links.\n`,
  );
}

const isMain =
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href;

if (isMain) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(
      `${error instanceof Error ? error.message : String(error)}\n`,
    );
    process.exitCode = 1;
  }
}
