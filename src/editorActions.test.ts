import { describe, expect, it } from "vitest";
import {
  needsUnsavedResolution,
  nextViewMode,
  shortcutAction,
  type DeferredAction,
} from "./editorActions";

describe("editor actions", () => {
  it.each<DeferredAction>([
    { kind: "open", topic: "git" },
    { kind: "create", topic: "rust", title: "Rust" },
    { kind: "rename", newTopic: "git/rebase" },
    { kind: "delete" },
    { kind: "chooseVault" },
    { kind: "close" },
  ])("requires a decision before $kind when work is unsaved", (action) => {
    expect(needsUnsavedResolution(true, action)).toBe(true);
    expect(needsUnsavedResolution(false, action)).toBe(false);
  });

  it("cycles through the three editor views", () => {
    expect(nextViewMode("edit")).toBe("split");
    expect(nextViewMode("split")).toBe("preview");
    expect(nextViewMode("preview")).toBe("edit");
  });

  it.each([
    ["k", false, "focusSearch"],
    ["n", false, "focusNewPage"],
    ["s", false, "save"],
    ["P", true, "cycleView"],
  ] as const)("maps Ctrl/Command+%s to %s", (key, shiftKey, expected) => {
    expect(
      shortcutAction({
        altKey: false,
        ctrlKey: true,
        key,
        metaKey: false,
        shiftKey,
      }),
    ).toBe(expected);
  });

  it("ignores unmodified and Alt-modified typing", () => {
    expect(
      shortcutAction({
        altKey: false,
        ctrlKey: false,
        key: "k",
        metaKey: false,
        shiftKey: false,
      }),
    ).toBeNull();
    expect(
      shortcutAction({
        altKey: true,
        ctrlKey: true,
        key: "k",
        metaKey: false,
        shiftKey: false,
      }),
    ).toBeNull();
  });
});
