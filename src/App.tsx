import {
  FormEvent,
  KeyboardEvent as ReactKeyboardEvent,
  ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createPortal } from "react-dom";
import "./App.css";
import {
  DeferredAction,
  needsUnsavedResolution,
  nextViewMode,
  shortcutAction,
  ViewMode,
} from "./editorActions";
import { MarkdownPreview } from "./MarkdownPreview";

type PageSummary = {
  topic: string;
  title: string;
  path: string;
};

type PageRevision = {
  modifiedUnixNanos: string | null;
  contentSha256: string;
};

type Page = PageSummary & {
  content: string;
  revision: PageRevision;
};

type DeletedPage = {
  topic: string;
  recoveryToken: string;
  recoveryPath: string;
};

type CommandError = {
  kind: string;
  message: string;
  draftPath?: string | null;
  actualRevision?: PageRevision | null;
};

type Conflict = {
  disk: Page | null;
  draftPath: string | null;
};

type EditorSnapshot = {
  selected: Page | null;
  draft: string;
  saving: boolean;
  conflict: Conflict | null;
};

type ModalState =
  | { kind: "unsaved"; action: DeferredAction }
  | { kind: "rename" }
  | { kind: "delete" }
  | { kind: "discardConflict" };

type ModalProps = {
  children: ReactNode;
  description: string;
  onCancel: () => void;
  title: string;
};

function parseCommandError(error: unknown): CommandError {
  if (
    typeof error === "object" &&
    error !== null &&
    "kind" in error &&
    "message" in error
  ) {
    return error as CommandError;
  }
  return { kind: "unknown", message: String(error) };
}

function sameRevision(left: PageRevision, right: PageRevision): boolean {
  return (
    left.modifiedUnixNanos === right.modifiedUnixNanos &&
    left.contentSha256 === right.contentSha256
  );
}

function pageTitle(topic: string): string {
  const leaf = topic.split("/").pop() ?? topic;
  return leaf
    .replace(/-/g, " ")
    .replace(/\b\w/g, (letter: string) => letter.toUpperCase());
}

function actionLabel(action: DeferredAction): string {
  switch (action.kind) {
    case "open":
      return `open ${action.topic}`;
    case "create":
      return `create ${action.topic}`;
    case "rename":
      return `rename this page to ${action.newTopic}`;
    case "delete":
      return "move this page to a recovery file";
    case "chooseVault":
      return "switch vaults";
    case "close":
      return "close MyHelp";
  }
}

function Modal({ children, description, onCancel, title }: ModalProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const previousFocusRef = useRef(
    typeof document === "undefined" ? null : (document.activeElement as HTMLElement | null),
  );

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    const target = dialog.querySelector<HTMLElement>("[data-autofocus]");
    (target ?? dialog).focus();
    return () => {
      const previousFocus = previousFocusRef.current;
      if (previousFocus?.isConnected) previousFocus.focus();
    };
  }, []);

  function trapFocus(event: ReactKeyboardEvent<HTMLDialogElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      onCancel();
      return;
    }
    if (event.key !== "Tab") return;

    const focusable = Array.from(
      event.currentTarget.querySelectorAll<HTMLElement>(
        'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    );
    if (focusable.length === 0) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  return createPortal(
    <div className="modal-backdrop">
      <dialog
        aria-describedby="modal-description"
        aria-labelledby="modal-title"
        aria-modal="true"
        className="modal"
        onKeyDown={trapFocus}
        open
        ref={dialogRef}
      >
        <p className="eyebrow">CONFIRM ACTION</p>
        <h2 id="modal-title">{title}</h2>
        <p id="modal-description">{description}</p>
        {children}
      </dialog>
    </div>,
    document.body,
  );
}

function App() {
  const [pages, setPages] = useState<PageSummary[]>([]);
  const [selected, setSelected] = useState<Page | null>(null);
  const [draft, setDraft] = useState("");
  const [query, setQuery] = useState("");
  const [newTopic, setNewTopic] = useState("");
  const [renameTopic, setRenameTopic] = useState("");
  const [vaultPath, setVaultPath] = useState("");
  const [status, setStatus] = useState("Loading your help vault…");
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [conflict, setConflict] = useState<Conflict | null>(null);
  const [modal, setModal] = useState<ModalState | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("split");
  const [lastDeleted, setLastDeleted] = useState<DeletedPage | null>(null);
  const queryRef = useRef("");
  const searchRef = useRef<HTMLInputElement>(null);
  const newTopicRef = useRef<HTMLInputElement>(null);
  const editorTextRef = useRef<HTMLTextAreaElement>(null);
  const editorRef = useRef<EditorSnapshot>({
    selected: null,
    draft: "",
    saving: false,
    conflict: null,
  });

  const dirty = selected !== null && draft !== selected.content;
  const hasUnsavedWork = dirty || conflict !== null;

  useEffect(() => {
    editorRef.current = { selected, draft, saving, conflict };
    queryRef.current = query;
  }, [selected, draft, saving, conflict, query]);

  useEffect(() => {
    void initialize();
  }, []);

  useEffect(() => {
    let timeout: number | undefined;
    const scheduleReconcile = () => {
      window.clearTimeout(timeout);
      timeout = window.setTimeout(() => void reconcileSelectedPage(), 180);
    };
    const unlistenChanged = listen("vault-changed", scheduleReconcile);
    const unlistenError = listen<{ message: string }>(
      "vault-watch-error",
      (event) => {
        setStatus(`Vault watcher warning: ${event.payload.message}`);
      },
    );
    window.addEventListener("focus", scheduleReconcile);
    document.addEventListener("visibilitychange", scheduleReconcile);

    return () => {
      window.clearTimeout(timeout);
      window.removeEventListener("focus", scheduleReconcile);
      document.removeEventListener("visibilitychange", scheduleReconcile);
      void unlistenChanged.then((unlisten) => unlisten());
      void unlistenError.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const registration = getCurrentWindow().onCloseRequested((event) => {
      const current = editorRef.current;
      const shouldConfirm =
        current.conflict !== null ||
        (current.selected !== null && current.draft !== current.selected.content);
      if (!shouldConfirm) return;
      event.preventDefault();
      setModal({ kind: "unsaved", action: { kind: "close" } });
    });
    void registration.then((registeredUnlisten) => {
      if (disposed) {
        registeredUnlisten();
      } else {
        unlisten = registeredUnlisten;
      }
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!hasUnsavedWork) return;
    const confirmUnload = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener("beforeunload", confirmUnload);
    return () => window.removeEventListener("beforeunload", confirmUnload);
  }, [hasUnsavedWork]);

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if (event.isComposing) return;
      const action = shortcutAction(event);
      if (!action) return;
      event.preventDefault();

      if (action === "focusSearch") {
        searchRef.current?.focus();
        searchRef.current?.select();
      } else if (action === "focusNewPage") {
        newTopicRef.current?.focus();
        newTopicRef.current?.select();
      } else if (action === "save") {
        void savePage();
      } else {
        setViewMode((current) => nextViewMode(current));
      }
    };
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [selected, draft, saving, conflict]);

  async function initialize() {
    setLoading(true);
    setLoadError(null);
    try {
      const [path, initialPages] = await Promise.all([
        invoke<string>("get_vault_path"),
        invoke<PageSummary[]>("list_pages"),
      ]);
      setVaultPath(path);
      setPages(initialPages);
      setStatus(
        initialPages.length === 0
          ? "Create your first help page."
          : `${initialPages.length} page${initialPages.length === 1 ? "" : "s"}`,
      );
      if (initialPages[0]) {
        await openPage(initialPages[0].topic);
      }
    } catch (error) {
      const message = parseCommandError(error).message;
      setLoadError(message);
      setStatus(`Could not open the vault: ${message}`);
    } finally {
      setLoading(false);
    }
  }

  async function loadPageList(search: string): Promise<PageSummary[]> {
    const nextPages = search.trim()
      ? await invoke<PageSummary[]>("search_pages", { query: search.trim() })
      : await invoke<PageSummary[]>("list_pages");
    setPages(nextPages);
    return nextPages;
  }

  async function refreshPages(search = query) {
    const nextPages = await loadPageList(search);
    setStatus(`${nextPages.length} matching page${nextPages.length === 1 ? "" : "s"}`);
  }

  async function openPage(topic: string) {
    try {
      const page = await invoke<Page>("read_page", { topic });
      setSelected(page);
      setDraft(page.content);
      setConflict(null);
      setStatus(`Editing ${page.topic}`);
      window.requestAnimationFrame(() => editorTextRef.current?.focus());
    } catch (error) {
      setStatus(`Could not open ${topic}: ${parseCommandError(error).message}`);
    }
  }

  async function savePage(page = selected, content = draft): Promise<Page | null> {
    if (!page || conflict || saving) return null;
    if (content === page.content) return page;

    setSaving(true);
    try {
      const saved = await invoke<Page>("save_page", {
        topic: page.topic,
        content,
        expectedRevision: page.revision,
      });
      setSelected(saved);
      setDraft(saved.content);
      await loadPageList(queryRef.current);
      setStatus(`Saved ${saved.topic}`);
      return saved;
    } catch (error) {
      const commandError = parseCommandError(error);
      if (commandError.kind === "conflict") {
        let disk: Page | null = null;
        try {
          disk = await invoke<Page>("read_page", { topic: page.topic });
        } catch (readError) {
          if (parseCommandError(readError).kind !== "notFound") {
            setStatus(
              `Conflict detected, but the disk version could not be read: ${
                parseCommandError(readError).message
              }`,
            );
          }
        }
        setConflict({
          disk,
          draftPath: commandError.draftPath ?? null,
        });
        setStatus(
          commandError.draftPath
            ? `Conflict: disk version kept; draft copied to ${commandError.draftPath}`
            : `Conflict: ${commandError.message}`,
        );
      } else {
        setStatus(`Could not save ${page.topic}: ${commandError.message}`);
      }
      return null;
    } finally {
      setSaving(false);
    }
  }

  function requestAction(action: DeferredAction) {
    if (editorRef.current.saving) {
      setStatus("Wait for the current save to finish before changing pages.");
      return;
    }
    if (needsUnsavedResolution(hasUnsavedWork, action)) {
      setModal({ kind: "unsaved", action });
      return;
    }
    void executeAction(action);
  }

  async function executeAction(action: DeferredAction, basePage?: Page | null) {
    if (action.kind === "open") {
      await openPage(action.topic);
      return;
    }
    if (action.kind === "create") {
      try {
        const page = await invoke<Page>("create_page", {
          topic: action.topic,
          title: action.title,
        });
        setNewTopic("");
        setQuery("");
        await loadPageList("");
        setSelected(page);
        setDraft(page.content);
        setConflict(null);
        setStatus(`Created ${page.topic}`);
        window.requestAnimationFrame(() => editorTextRef.current?.focus());
      } catch (error) {
        setStatus(
          `Could not create ${action.topic}: ${parseCommandError(error).message}`,
        );
      }
      return;
    }
    if (action.kind === "chooseVault") {
      try {
        const path = await invoke<string | null>("choose_vault");
        if (!path) {
          setStatus("Vault selection cancelled.");
          return;
        }
        setVaultPath(path);
        setQuery("");
        setSelected(null);
        setDraft("");
        setConflict(null);
        setLastDeleted(null);
        await initialize();
        setStatus(`Opened vault ${path}`);
      } catch (error) {
        setStatus(`Could not switch vaults: ${parseCommandError(error).message}`);
      }
      return;
    }
    if (action.kind === "close") {
      try {
        await invoke("close_window");
      } catch (error) {
        setStatus(`Could not close MyHelp: ${parseCommandError(error).message}`);
      }
      return;
    }

    const page = basePage ?? editorRef.current.selected;
    if (!page) {
      setStatus("The page is no longer available.");
      return;
    }
    if (action.kind === "rename") {
      try {
        const renamed = await invoke<Page>("rename_page", {
          topic: page.topic,
          newTopic: action.newTopic,
          expectedRevision: page.revision,
        });
        setSelected(renamed);
        setDraft(renamed.content);
        setConflict(null);
        setQuery("");
        await loadPageList("");
        setStatus(`Renamed ${page.topic} to ${renamed.topic}`);
      } catch (error) {
        setStatus(`Could not rename ${page.topic}: ${parseCommandError(error).message}`);
      }
      return;
    }

    try {
      const deleted = await invoke<DeletedPage>("delete_page", {
        topic: page.topic,
        expectedRevision: page.revision,
      });
      setLastDeleted(deleted);
      setSelected(null);
      setDraft("");
      setConflict(null);
      await loadPageList(queryRef.current);
      setStatus(`Moved ${page.topic} to a recovery file. Undo is available.`);
    } catch (error) {
      setStatus(`Could not delete ${page.topic}: ${parseCommandError(error).message}`);
    }
  }

  async function resolveUnsaved(saveFirst: boolean) {
    if (modal?.kind !== "unsaved") return;
    const action = modal.action;
    const current = editorRef.current;
    let basePage = current.conflict?.disk ?? current.selected;

    if (saveFirst) {
      if (current.conflict) {
        setStatus("Resolve the external change before saving.");
        setModal(null);
        return;
      }
      basePage = await savePage(current.selected, current.draft);
      if (!basePage) {
        setModal(null);
        return;
      }
    } else if (
      current.conflict?.disk === null &&
      (action.kind === "rename" || action.kind === "delete")
    ) {
      setStatus("The page was already deleted on disk; choose Restore or accept deletion.");
      setModal(null);
      return;
    }

    setModal(null);
    if (!saveFirst && current.selected) {
      setDraft(basePage?.content ?? current.selected.content);
      setConflict(null);
    }
    await executeAction(action, basePage);
  }

  async function undoDelete() {
    if (!lastDeleted) return;
    try {
      const page = await invoke<Page>("restore_deleted_page", {
        topic: lastDeleted.topic,
        recoveryToken: lastDeleted.recoveryToken,
      });
      setLastDeleted(null);
      setSelected(page);
      setDraft(page.content);
      setQuery("");
      await loadPageList("");
      setStatus(`Restored ${page.topic}`);
    } catch (error) {
      setStatus(
        `Could not restore ${lastDeleted.topic}: ${parseCommandError(error).message}`,
      );
    }
  }

  async function reconcileSelectedPage() {
    const before = editorRef.current;
    if (!before.selected || before.saving) return;

    let disk: Page | null;
    try {
      disk = await invoke<Page>("read_page", { topic: before.selected.topic });
    } catch (error) {
      const commandError = parseCommandError(error);
      if (commandError.kind !== "notFound") {
        setStatus(`Could not refresh ${before.selected.topic}: ${commandError.message}`);
        return;
      }
      disk = null;
    }

    const latest = editorRef.current;
    if (
      !latest.selected ||
      latest.selected.topic !== before.selected.topic ||
      !sameRevision(latest.selected.revision, before.selected.revision)
    ) {
      return;
    }
    if (disk && sameRevision(disk.revision, latest.selected.revision)) return;

    const hasDraft = latest.draft !== latest.selected.content;
    if (!hasDraft) {
      if (disk) {
        setSelected(disk);
        setDraft(disk.content);
        setConflict(null);
        setStatus(`Reloaded external changes to ${disk.topic}`);
      } else {
        setSelected(null);
        setDraft("");
        setConflict(null);
        setStatus(`${latest.selected.topic} was removed outside MyHelp`);
      }
      await loadPageList(queryRef.current);
      return;
    }

    if (
      latest.conflict &&
      ((latest.conflict.disk === null && disk === null) ||
        (latest.conflict.disk !== null &&
          disk !== null &&
          sameRevision(latest.conflict.disk.revision, disk.revision)))
    ) {
      return;
    }

    let draftPath: string | null = null;
    try {
      draftPath = await invoke<string>("preserve_draft", {
        topic: latest.selected.topic,
        content: latest.draft,
      });
    } catch (error) {
      setStatus(
        `External change detected; the draft is still open but could not be copied: ${
          parseCommandError(error).message
        }`,
      );
    }
    setConflict({ disk, draftPath });
    if (draftPath) {
      setStatus(
        `External change detected; disk version kept and draft copied to ${draftPath}`,
      );
    }
    await loadPageList(queryRef.current);
  }

  function useDiskAsSaveBase() {
    if (!conflict?.disk) return;
    setSelected(conflict.disk);
    setConflict(null);
    setStatus("Disk revision accepted as the save base. Review the draft, then save.");
    window.requestAnimationFrame(() => editorTextRef.current?.focus());
  }

  function applyDiskVersion() {
    if (!conflict) return;
    if (conflict.disk) {
      setSelected(conflict.disk);
      setDraft(conflict.disk.content);
      setStatus(`Loaded the disk version of ${conflict.disk.topic}`);
    } else {
      setSelected(null);
      setDraft("");
      setStatus("Accepted the external deletion.");
    }
    setConflict(null);
    setModal(null);
  }

  function loadDiskVersion() {
    if (!conflict) return;
    if (!conflict.draftPath) {
      setModal({ kind: "discardConflict" });
      return;
    }
    applyDiskVersion();
  }

  async function restoreDeletedDraft() {
    if (!selected || conflict?.disk) return;
    try {
      const restored = await invoke<Page>("restore_page", {
        topic: selected.topic,
        content: draft,
      });
      setSelected(restored);
      setDraft(restored.content);
      setConflict(null);
      await loadPageList(queryRef.current);
      setStatus(`Restored ${restored.topic} from the preserved draft`);
    } catch (error) {
      setStatus(`Could not restore the page: ${parseCommandError(error).message}`);
    }
  }

  async function search(event: FormEvent) {
    event.preventDefault();
    try {
      await refreshPages(query);
    } catch (error) {
      setStatus(`Search failed: ${parseCommandError(error).message}`);
    }
  }

  function createPage(event: FormEvent) {
    event.preventDefault();
    const topic = newTopic.trim();
    if (!topic) return;
    requestAction({ kind: "create", topic, title: pageTitle(topic) });
  }

  function submitRename(event: FormEvent) {
    event.preventDefault();
    const newTopic = renameTopic.trim();
    if (!newTopic || !selected) return;
    setModal(null);
    requestAction({ kind: "rename", newTopic });
  }

  function startRename() {
    if (!selected) return;
    setRenameTopic(selected.topic);
    setModal({ kind: "rename" });
  }

  const editorVisible = viewMode !== "preview";
  const previewVisible = viewMode !== "edit";

  return (
    <main
      aria-hidden={modal !== null ? true : undefined}
      className="app-shell"
      inert={modal !== null ? true : undefined}
    >
      <a className="skip-link" href="#editor-workspace">
        Skip to editor
      </a>
      <header className="topbar">
        <div>
          <p className="eyebrow">LOCAL-FIRST HELP VAULT</p>
          <h1>MyHelp</h1>
        </div>
        <div className="topbar-actions">
          <span className="vault-path" title={vaultPath}>
            {vaultPath || "Resolving vault…"}
          </span>
          <button
            aria-label="Choose another vault"
            className="secondary compact"
            disabled={loading}
            onClick={() => requestAction({ kind: "chooseVault" })}
            type="button"
          >
            Choose vault
          </button>
          <button
            aria-keyshortcuts="Control+S Meta+S"
            className="primary"
            disabled={!dirty || saving || conflict !== null}
            onClick={() => void savePage()}
            type="button"
          >
            {saving
              ? "Saving…"
              : conflict
                ? "Resolve conflict"
                : dirty
                  ? "Save changes"
                  : "Saved"}
          </button>
        </div>
      </header>

      <div className="workspace">
        <aside aria-label="Vault pages" className="sidebar">
          <form className="search" onSubmit={(event) => void search(event)}>
            <label htmlFor="search-pages">
              Search pages <kbd>Ctrl/⌘ K</kbd>
            </label>
            <div className="input-row">
              <input
                aria-keyshortcuts="Control+K Meta+K"
                id="search-pages"
                onChange={(event) => setQuery(event.currentTarget.value)}
                placeholder="Python, Nix, Git…"
                ref={searchRef}
                value={query}
              />
              <button type="submit">Search</button>
            </div>
          </form>

          <nav aria-busy={loading} aria-label="Help pages" className="page-list">
            {loading ? (
              <p className="empty">Loading pages…</p>
            ) : (
              pages.map((page) => (
                <button
                  aria-label={`Open ${page.topic}`}
                  aria-current={selected?.topic === page.topic ? "page" : undefined}
                  className={selected?.topic === page.topic ? "page active" : "page"}
                  key={page.topic}
                  onClick={() => requestAction({ kind: "open", topic: page.topic })}
                  type="button"
                >
                  <strong>{page.title}</strong>
                  <span>{page.topic}</span>
                </button>
              ))
            )}
            {!loading && pages.length === 0 && (
              <p className="empty">
                {query.trim()
                  ? "No pages match this search."
                  : "No pages yet. Create one below."}
              </p>
            )}
          </nav>

          <form className="new-page" onSubmit={createPage}>
            <label htmlFor="new-topic">
              New page <kbd>Ctrl/⌘ N</kbd>
            </label>
            <input
              aria-keyshortcuts="Control+N Meta+N"
              id="new-topic"
              onChange={(event) => setNewTopic(event.currentTarget.value)}
              placeholder="python/new-project"
              ref={newTopicRef}
              value={newTopic}
            />
            <button className="secondary" type="submit" disabled={!newTopic.trim()}>
              Create page
            </button>
          </form>
        </aside>

        <section
          aria-busy={loading}
          className="editor-area"
          id="editor-workspace"
          tabIndex={-1}
        >
          {loadError ? (
            <div className="state-card" role="alert">
              <p className="eyebrow">VAULT UNAVAILABLE</p>
              <h2>MyHelp could not open this vault.</h2>
              <p>{loadError}</p>
              <div className="button-row">
                <button className="primary" onClick={() => void initialize()}>
                  Retry
                </button>
                <button
                  className="secondary"
                  onClick={() => requestAction({ kind: "chooseVault" })}
                >
                  Choose another vault
                </button>
              </div>
            </div>
          ) : selected ? (
            <>
              <div className="document-heading">
                <div>
                  <p className="eyebrow">TOPIC</p>
                  <h2>{selected.topic}</h2>
                </div>
                <div className="document-actions">
                  {dirty && <span className="unsaved">Unsaved</span>}
                  <button className="secondary compact" onClick={startRename} type="button">
                    Rename
                  </button>
                  <button
                    className="danger compact"
                    onClick={() => setModal({ kind: "delete" })}
                    type="button"
                  >
                    Delete
                  </button>
                </div>
              </div>

              {conflict && (
                <section className="conflict-banner" role="alert">
                  <div>
                    <p className="eyebrow">EXTERNAL CHANGE</p>
                    <h3>
                      {conflict.disk
                        ? "This page changed on disk."
                        : "This page was deleted on disk."}
                    </h3>
                    <p>
                      The disk version was not overwritten. Your current draft remains
                      in the editor
                      {conflict.draftPath ? (
                        <>
                          {" "}
                          and is also saved at <code>{conflict.draftPath}</code>
                        </>
                      ) : (
                        "."
                      )}
                    </p>
                  </div>
                  <div className="conflict-actions">
                    {conflict.disk ? (
                      <>
                        <button
                          className="primary"
                          onClick={useDiskAsSaveBase}
                          type="button"
                        >
                          Reconcile and save draft
                        </button>
                        <button
                          className="secondary"
                          onClick={loadDiskVersion}
                          type="button"
                        >
                          Load disk version
                        </button>
                      </>
                    ) : (
                      <>
                        <button
                          className="primary"
                          onClick={() => void restoreDeletedDraft()}
                          type="button"
                        >
                          Restore draft as page
                        </button>
                        <button
                          className="secondary"
                          onClick={loadDiskVersion}
                          type="button"
                        >
                          Accept deletion
                        </button>
                      </>
                    )}
                  </div>
                  {conflict.disk && (
                    <details>
                      <summary>Review the disk version before reconciling</summary>
                      <pre>{conflict.disk.content}</pre>
                    </details>
                  )}
                </section>
              )}

              <div className="view-switcher" role="group" aria-label="Editor view">
                {(["edit", "split", "preview"] as const).map((mode) => (
                  <button
                    aria-pressed={viewMode === mode}
                    className={viewMode === mode ? "active" : ""}
                    key={mode}
                    onClick={() => setViewMode(mode)}
                    type="button"
                  >
                    {mode === "edit" ? "Editor" : mode === "split" ? "Split" : "Preview"}
                  </button>
                ))}
                <span className="shortcut-hint">
                  Cycle <kbd>Ctrl/⌘ ⇧ P</kbd>
                </span>
              </div>

              <div
                className={`split-editor view-${viewMode}`}
                id="editor-panes"
              >
                <section aria-hidden={!editorVisible} className="pane editor-pane">
                  <h3>Markdown</h3>
                  <textarea
                    aria-label={`Edit ${selected.topic}`}
                    ref={editorTextRef}
                    onChange={(event) => setDraft(event.currentTarget.value)}
                    spellCheck
                    tabIndex={editorVisible ? 0 : -1}
                    value={draft}
                  />
                </section>
                <section aria-hidden={!previewVisible} className="pane preview">
                  <h3>Preview</h3>
                  <article>
                    <MarkdownPreview source={draft} />
                  </article>
                </section>
              </div>
            </>
          ) : (
            <div className="welcome">
              <p className="eyebrow">PLAIN MARKDOWN, EVERYWHERE</p>
              <h2>Your commands should be one search away.</h2>
              <p>
                Create a page from the sidebar, then read the same file from the
                desktop app, the CLI, or a tldr-compatible client.
              </p>
              <button
                className="primary"
                onClick={() => newTopicRef.current?.focus()}
                type="button"
              >
                Create your first page
              </button>
            </div>
          )}
        </section>
      </div>

      {lastDeleted && (
        <aside className="undo-banner" aria-label="Deleted page recovery">
          <span>
            <strong>{lastDeleted.topic}</strong> is in a readable recovery file.
          </span>
          <button className="secondary compact" onClick={() => void undoDelete()}>
            Undo delete
          </button>
        </aside>
      )}
      <div aria-atomic="true" aria-live="polite" className="statusbar" role="status">
        {status}
      </div>

      {modal?.kind === "unsaved" && (
        <Modal
          description={`You have work that has not been committed to the current page. Decide what to do before you ${actionLabel(modal.action)}.`}
          onCancel={() => setModal(null)}
          title="Keep your current changes?"
        >
          <div className="modal-actions">
            <button
              className="primary"
              data-autofocus
              disabled={conflict !== null}
              onClick={() => void resolveUnsaved(true)}
              type="button"
            >
              Save and continue
            </button>
            <button
              className="danger"
              onClick={() => void resolveUnsaved(false)}
              type="button"
            >
              Discard draft and continue
            </button>
            <button className="secondary" onClick={() => setModal(null)} type="button">
              Cancel
            </button>
          </div>
          {conflict && (
            <p className="modal-note">
              Saving is unavailable until the external change is resolved. You can
              cancel and use the conflict controls, or discard this draft.
            </p>
          )}
        </Modal>
      )}

      {modal?.kind === "rename" && selected && (
        <Modal
          description="The Markdown page and its optional metadata sidecar move together. An existing destination is never overwritten."
          onCancel={() => setModal(null)}
          title={`Rename ${selected.topic}`}
        >
          <form className="modal-form" onSubmit={submitRename}>
            <label htmlFor="rename-topic">New topic</label>
            <input
              data-autofocus
              id="rename-topic"
              onChange={(event) => setRenameTopic(event.currentTarget.value)}
              value={renameTopic}
            />
            <div className="modal-actions">
              <button
                className="primary"
                disabled={!renameTopic.trim() || renameTopic.trim() === selected.topic}
                type="submit"
              >
                Rename page
              </button>
              <button className="secondary" onClick={() => setModal(null)} type="button">
                Cancel
              </button>
            </div>
          </form>
        </Modal>
      )}

      {modal?.kind === "delete" && selected && (
        <Modal
          description={`${selected.topic} will leave the page list, but MyHelp keeps a readable recovery Markdown file so you can undo the deletion.`}
          onCancel={() => setModal(null)}
          title={`Delete ${selected.topic}?`}
        >
          <div className="modal-actions">
            <button
              className="danger"
              data-autofocus
              onClick={() => {
                setModal(null);
                requestAction({ kind: "delete" });
              }}
              type="button"
            >
              Move to recovery file
            </button>
            <button className="secondary" onClick={() => setModal(null)} type="button">
              Cancel
            </button>
          </div>
        </Modal>
      )}

      {modal?.kind === "discardConflict" && (
        <Modal
          description="The draft could not be copied to disk. Loading the disk version now will permanently discard the only remaining in-memory copy."
          onCancel={() => setModal(null)}
          title="Discard the in-memory draft?"
        >
          <div className="modal-actions">
            <button
              className="danger"
              data-autofocus
              onClick={applyDiskVersion}
              type="button"
            >
              Discard in-memory draft
            </button>
            <button className="secondary" onClick={() => setModal(null)} type="button">
              Keep editing
            </button>
          </div>
        </Modal>
      )}
    </main>
  );
}

export default App;
