// @vitest-environment jsdom

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const mocks = vi.hoisted(() => ({
  closeListener: null as
    | ((event: { preventDefault: () => void }) => void)
    | null,
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: async (
      listener: (event: { preventDefault: () => void }) => void,
    ) => {
      mocks.closeListener = listener;
      return () => {
        mocks.closeListener = null;
      };
    },
  }),
}));

const gitPage = {
  topic: "git",
  title: "Git",
  path: "/vault/git.page.md",
  content: "# Git\n",
  revision: {
    modifiedUnixNanos: "1",
    contentSha256: "a".repeat(64),
  },
};

const rustPage = {
  topic: "rust",
  title: "Rust",
  path: "/vault/rust.page.md",
  content: "# Rust\n",
  revision: {
    modifiedUnixNanos: "2",
    contentSha256: "b".repeat(64),
  },
};

function configureBackend() {
  mocks.invoke.mockImplementation(async (command: string, arguments_: unknown) => {
    const argumentsObject = (arguments_ ?? {}) as Record<string, unknown>;
    if (command === "get_vault_path") return "/vault";
    if (command === "list_pages") {
      return [gitPage, rustPage].map(({ content: _content, revision: _revision, ...page }) => page);
    }
    if (command === "read_page") {
      return argumentsObject.topic === "rust" ? rustPage : gitPage;
    }
    if (command === "save_page") {
      return {
        ...gitPage,
        content: argumentsObject.content,
        revision: {
          modifiedUnixNanos: "3",
          contentSha256: "c".repeat(64),
        },
      };
    }
    if (command === "delete_page") {
      return {
        topic: "git",
        recoveryToken: "a".repeat(64),
        recoveryPath: `/vault/git.page.deleted-${"a".repeat(64)}.md`,
      };
    }
    if (command === "restore_deleted_page") return gitPage;
    if (command === "close_window") return undefined;
    throw new Error(`unexpected command: ${command}`);
  });
}

beforeEach(() => {
  mocks.invoke.mockReset();
  configureBackend();
});

afterEach(() => {
  cleanup();
  mocks.closeListener = null;
});

describe("desktop editor workflow", () => {
  it("supports primary focus shortcuts and announces a clean initial UI", async () => {
    const user = userEvent.setup();
    const { container } = render(<App />);
    await screen.findByRole("textbox", { name: "Edit git" });

    await user.keyboard("{Control>}k{/Control}");
    expect(document.activeElement).toBe(
      screen.getByRole("textbox", { name: /^Search pages/ }),
    );

    await user.keyboard("{Control>}n{/Control}");
    expect(document.activeElement).toBe(
      screen.getByRole("textbox", { name: /^New page/ }),
    );

    const result = await axe.run(container, {
      rules: {
        // jsdom has no canvas/layout implementation; contrast is verified from
        // the shared palette and in the manual screen-reader/zoom checklist.
        "color-contrast": { enabled: false },
      },
    });
    expect(result.violations).toEqual([]);
  });

  it("requires an explicit decision before dirty navigation", async () => {
    const user = userEvent.setup();
    render(<App />);
    const editor = await screen.findByRole("textbox", { name: "Edit git" });
    await user.type(editor, "local draft");

    await user.click(screen.getByRole("button", { name: "Open rust" }));
    const dialog = screen.getByRole("dialog", {
      name: "Keep your current changes?",
    });
    expect(dialog).toBeTruthy();
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Save and continue" }),
    );

    await user.keyboard("{Shift>}{Tab}{/Shift}");
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Cancel" }),
    );

    const result = await axe.run(document.body, {
      rules: { "color-contrast": { enabled: false } },
    });
    expect(result.violations).toEqual([]);

    await user.click(
      screen.getByRole("button", { name: "Discard draft and continue" }),
    );
    await screen.findByRole("textbox", { name: "Edit rust" });
  });

  it("intercepts a dirty native close request", async () => {
    const user = userEvent.setup();
    render(<App />);
    const editor = await screen.findByRole("textbox", { name: "Edit git" });
    await user.type(editor, "local draft");

    const preventDefault = vi.fn();
    await act(async () => {
      mocks.closeListener?.({ preventDefault });
    });

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(
      screen.getByRole("dialog", { name: "Keep your current changes?" }),
    ).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Discard draft and continue" }));
    await waitFor(() =>
      expect(mocks.invoke).toHaveBeenCalledWith("close_window"),
    );
  });

  it("makes deletion recoverable from the editor", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("textbox", { name: "Edit git" });

    await user.click(screen.getByRole("button", { name: "Delete" }));
    await user.click(screen.getByRole("button", { name: "Move to recovery file" }));
    await screen.findByRole("button", { name: "Undo delete" });

    await user.click(screen.getByRole("button", { name: "Undo delete" }));
    await screen.findByRole("textbox", { name: "Edit git" });
    expect(mocks.invoke).toHaveBeenCalledWith("restore_deleted_page", {
      topic: "git",
      recoveryToken: "a".repeat(64),
    });
  });
});
