import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { validateLanguageWorkflowPack } from "./language-workflows.mjs";

const DOCUMENTATION = "https://example.com/tool/docs";

function fixture() {
  const directory = mkdtempSync(join(tmpdir(), "myhelp-workflows-test-"));
  writeFileSync(
    join(directory, "sample-new-project.page.md"),
    `# Sample new project\n\n> Create a sample project.\n> Official documentation: <${DOCUMENTATION}>.\n\n- Create a project:\n\n\`sample new {{project}}\`\n`,
    "utf8",
  );
  writeFileSync(
    join(directory, "manifest.json"),
    `${JSON.stringify(
      {
        schemaVersion: 1,
        reviewedOn: "2026-07-22",
        pages: [
          {
            file: "sample-new-project.page.md",
            topic: "sample-new-project",
            toolchain: "Sample 1.0",
            documentation: [DOCUMENTATION],
          },
        ],
      },
      null,
      2,
    )}\n`,
    "utf8",
  );
  return directory;
}

test("accepts a complete portable starter pack", () => {
  const directory = fixture();
  try {
    assert.deepEqual(validateLanguageWorkflowPack(directory), {
      pageCount: 1,
      documentationLinkCount: 1,
    });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("rejects undocumented and unreviewed dependency updates", () => {
  const directory = fixture();
  try {
    writeFileSync(
      join(directory, "sample-new-project.page.md"),
      "# Sample new project\n\n> Create a sample project.\n\n- Update dependencies:\n\n`pnpm update`\n",
      "utf8",
    );
    assert.throws(
      () => validateLanguageWorkflowPack(directory),
      /official documentation line[\s\S]*dependency update must tell the reader to review/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("rejects machine-specific paths and unlabeled destructive commands", () => {
  const directory = fixture();
  try {
    writeFileSync(
      join(directory, "sample-new-project.page.md"),
      `# Sample new project\n\n> Create a sample project.\n> Official documentation: <${DOCUMENTATION}>.\n\n- Clean /home/alice/project:\n\n\`rm -rf /home/alice/project\`\n`,
      "utf8",
    );
    assert.throws(
      () => validateLanguageWorkflowPack(directory),
      /machine-specific home path[\s\S]*destructive command must be labeled Destructive/,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
