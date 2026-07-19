import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./App.css", import.meta.url), "utf8");

function palette(block: string): Record<string, string> {
  return Object.fromEntries(
    Array.from(
      block.matchAll(/--color-([\w-]+):\s*(#[\da-f]{6});/g),
      (match) => [match[1], match[2]],
    ),
  );
}

function luminance(hex: string): number {
  const channel = (offset: number) => {
    const value = Number.parseInt(hex.slice(offset, offset + 2), 16) / 255;
    return value <= 0.04045
      ? value / 12.92
      : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

function contrast(left: string, right: string): number {
  const leftLuminance = luminance(left);
  const rightLuminance = luminance(right);
  return (
    (Math.max(leftLuminance, rightLuminance) + 0.05) /
    (Math.min(leftLuminance, rightLuminance) + 0.05)
  );
}

describe("design tokens", () => {
  it("keeps component font sizes on named type roles", () => {
    const componentFontSizes = css
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.startsWith("font-size:"));

    expect(componentFontSizes.length).toBeGreaterThan(10);
    for (const declaration of componentFontSizes) {
      expect(declaration).toMatch(/^font-size: var\(--text-/);
    }
  });

  it("keeps accessibility adaptations in the shared stylesheet", () => {
    expect(css).toContain(":focus-visible");
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
    expect(css).toContain("@media (forced-colors: active)");
  });

  it("keeps semantic text pairs above WCAG AA normal-text contrast", () => {
    const rootEnd = css.indexOf("\n}");
    const light = palette(css.slice(css.indexOf(":root"), rootEnd));
    const darkStart = css.indexOf("@media (prefers-color-scheme: dark)");
    const dark = palette(css.slice(darkStart));
    const pairs = [
      ["text", "canvas"],
      ["text-muted", "canvas"],
      ["text-subtle", "canvas"],
      ["primary-text", "accent"],
      ["warning", "warning-surface"],
      ["danger", "danger-surface"],
      ["accent-text", "accent-soft"],
    ] as const;

    for (const current of [light, dark]) {
      for (const [foreground, background] of pairs) {
        expect(
          contrast(current[foreground], current[background]),
          `${foreground} on ${background}`,
        ).toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});
