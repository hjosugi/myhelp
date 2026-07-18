import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(root, "assets", "brand", "myhelp-mark.svg");
const target = join(root, "src-tauri", "icons");
const generated = mkdtempSync(join(tmpdir(), "myhelp-icons-"));
const checkOnly = process.argv.includes("--check");

const desktopIcons = [
  "32x32.png",
  "64x64.png",
  "128x128.png",
  "128x128@2x.png",
  "icon.png",
  "icon.icns",
  "icon.ico",
  "StoreLogo.png",
  "Square30x30Logo.png",
  "Square44x44Logo.png",
  "Square71x71Logo.png",
  "Square89x89Logo.png",
  "Square107x107Logo.png",
  "Square142x142Logo.png",
  "Square150x150Logo.png",
  "Square284x284Logo.png",
  "Square310x310Logo.png",
];
const bundleIcons = [
  "icons/32x32.png",
  "icons/128x128.png",
  "icons/128x128@2x.png",
  "icons/icon.icns",
  "icons/icon.ico",
];

function fail(message) {
  console.error(message);
  process.exitCode = 1;
}

function normalizeIcns(path) {
  const input = readFileSync(path);
  if (input.length < 8 || input.toString("ascii", 0, 4) !== "icns") {
    throw new Error(`Invalid ICNS header: ${path}`);
  }
  if (input.readUInt32BE(4) !== input.length) {
    throw new Error(`Invalid ICNS length: ${path}`);
  }

  const chunks = [];
  for (let offset = 8; offset < input.length; ) {
    if (offset + 8 > input.length) {
      throw new Error(`Truncated ICNS chunk: ${path}`);
    }
    const chunkLength = input.readUInt32BE(offset + 4);
    if (chunkLength < 8 || offset + chunkLength > input.length) {
      throw new Error(`Invalid ICNS chunk length: ${path}`);
    }
    chunks.push(input.subarray(offset, offset + chunkLength));
    offset += chunkLength;
  }
  chunks.sort((left, right) => {
    const leftType = left.toString("ascii", 0, 4);
    const rightType = right.toString("ascii", 0, 4);
    if (leftType === rightType) {
      return Buffer.compare(left, right);
    }
    return leftType < rightType ? -1 : 1;
  });

  const output = Buffer.concat([input.subarray(0, 8), ...chunks]);
  output.writeUInt32BE(output.length, 4);
  writeFileSync(path, output);
}

function checkTauriBrandConfig() {
  const config = JSON.parse(
    readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"),
  );
  if (config.productName !== "MyHelp") {
    throw new Error('Tauri productName must be "MyHelp"');
  }
  if (
    !Array.isArray(config.app?.windows) ||
    config.app.windows.length === 0 ||
    config.app.windows.some((window) => window.title !== "MyHelp")
  ) {
    throw new Error('Every Tauri window title must be "MyHelp"');
  }

  const configuredIcons = config.bundle?.icon;
  if (
    !Array.isArray(configuredIcons) ||
    configuredIcons.length !== bundleIcons.length ||
    bundleIcons.some((icon) => !configuredIcons.includes(icon))
  ) {
    throw new Error(
      `Tauri bundle icons must be: ${bundleIcons.join(", ")}`,
    );
  }
}

try {
  const tauriBinary = join(
    root,
    "node_modules",
    ".bin",
    process.platform === "win32" ? "tauri.cmd" : "tauri",
  );
  const result = spawnSync(
    tauriBinary,
    ["icon", source, "--output", generated],
    {
      cwd: root,
      stdio: "inherit",
      shell: process.platform === "win32",
    },
  );

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`Tauri icon generation exited with status ${result.status}`);
  }

  normalizeIcns(join(generated, "icon.icns"));
  checkTauriBrandConfig();

  const unexpected = readdirSync(target, { withFileTypes: true })
    .map((entry) => entry.name)
    .filter((name) => !desktopIcons.includes(name));
  if (unexpected.length > 0) {
    throw new Error(
      `Unexpected files in src-tauri/icons: ${unexpected.join(", ")}`,
    );
  }

  const stale = [];
  for (const icon of desktopIcons) {
    const generatedIcon = join(generated, icon);
    const targetIcon = join(target, icon);
    if (!existsSync(generatedIcon)) {
      throw new Error(`Tauri did not generate expected desktop icon: ${icon}`);
    }

    if (checkOnly) {
      if (
        !existsSync(targetIcon) ||
        !readFileSync(generatedIcon).equals(readFileSync(targetIcon))
      ) {
        stale.push(icon);
      }
    } else {
      mkdirSync(target, { recursive: true });
      copyFileSync(generatedIcon, targetIcon);
    }
  }

  if (stale.length > 0) {
    fail(`Generated icons are stale: ${stale.join(", ")}. Run pnpm icons.`);
  } else {
    console.log(
      checkOnly
        ? "All desktop icons match assets/brand/myhelp-mark.svg."
        : `Generated ${desktopIcons.length} desktop icons.`,
    );
  }
} finally {
  rmSync(generated, { recursive: true, force: true });
}
