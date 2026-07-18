import { FormEvent, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
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

function App() {
  const [pages, setPages] = useState<PageSummary[]>([]);
  const [selected, setSelected] = useState<Page | null>(null);
  const [draft, setDraft] = useState("");
  const [query, setQuery] = useState("");
  const [newTopic, setNewTopic] = useState("");
  const [vaultPath, setVaultPath] = useState("");
  const [status, setStatus] = useState("Loading your help vault…");
  const [saving, setSaving] = useState(false);
  const [conflict, setConflict] = useState<Conflict | null>(null);
  const queryRef = useRef("");
  const editorRef = useRef<EditorSnapshot>({
    selected: null,
    draft: "",
    saving: false,
    conflict: null,
  });

  const dirty = selected !== null && draft !== selected.content;

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

  async function initialize() {
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
      setStatus(`Could not open the vault: ${parseCommandError(error).message}`);
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
    const current = editorRef.current;
    const hasDraft =
      current.selected !== null && current.draft !== current.selected.content;
    if (hasDraft && !window.confirm("Discard the unsaved changes?")) {
      return;
    }

    try {
      const page = await invoke<Page>("read_page", { topic });
      setSelected(page);
      setDraft(page.content);
      setConflict(null);
      setStatus(`Editing ${page.topic}`);
    } catch (error) {
      setStatus(`Could not open ${topic}: ${parseCommandError(error).message}`);
    }
  }

  async function createPage(event: FormEvent) {
    event.preventDefault();
    const topic = newTopic.trim();
    if (!topic) return;
    const leaf = topic.split("/").pop() ?? topic;

    try {
      const page = await invoke<Page>("create_page", {
        topic,
        title: leaf
          .replace(/-/g, " ")
          .replace(/\b\w/g, (letter: string) => letter.toUpperCase()),
      });
      setNewTopic("");
      setQuery("");
      await loadPageList("");
      setSelected(page);
      setDraft(page.content);
      setConflict(null);
      setStatus(`Created ${page.topic}`);
    } catch (error) {
      setStatus(`Could not create ${topic}: ${parseCommandError(error).message}`);
    }
  }

  async function savePage() {
    if (!selected || conflict) return;

    setSaving(true);
    try {
      const page = await invoke<Page>("save_page", {
        topic: selected.topic,
        content: draft,
        expectedRevision: selected.revision,
      });
      setSelected(page);
      await loadPageList(queryRef.current);
      setStatus(`Saved ${page.topic}`);
    } catch (error) {
      const commandError = parseCommandError(error);
      if (commandError.kind === "conflict") {
        let disk: Page | null = null;
        try {
          disk = await invoke<Page>("read_page", { topic: selected.topic });
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
        setStatus(`Could not save ${selected.topic}: ${commandError.message}`);
      }
    } finally {
      setSaving(false);
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

    if (disk && sameRevision(disk.revision, latest.selected.revision)) {
      return;
    }

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
  }

  function loadDiskVersion() {
    if (!conflict) return;
    if (
      !conflict.draftPath &&
      !window.confirm("The draft copy failed. Discard the in-memory draft anyway?")
    ) {
      return;
    }
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

  return (
    <main className="app-shell">
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
            className="primary"
            disabled={!dirty || saving || conflict !== null}
            onClick={() => void savePage()}
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
        <aside className="sidebar">
          <form className="search" onSubmit={(event) => void search(event)}>
            <label htmlFor="search-pages">Search pages</label>
            <div className="input-row">
              <input
                id="search-pages"
                value={query}
                onChange={(event) => setQuery(event.currentTarget.value)}
                placeholder="Python, Nix, Git…"
              />
              <button type="submit" aria-label="Search">
                Search
              </button>
            </div>
          </form>

          <nav aria-label="Help pages" className="page-list">
            {pages.map((page) => (
              <button
                className={selected?.topic === page.topic ? "page active" : "page"}
                key={page.topic}
                onClick={() => void openPage(page.topic)}
              >
                <strong>{page.title}</strong>
                <span>{page.topic}</span>
              </button>
            ))}
            {pages.length === 0 && (
              <p className="empty">No pages match this search.</p>
            )}
          </nav>

          <form className="new-page" onSubmit={(event) => void createPage(event)}>
            <label htmlFor="new-topic">New page</label>
            <input
              id="new-topic"
              value={newTopic}
              onChange={(event) => setNewTopic(event.currentTarget.value)}
              placeholder="python/new-project"
            />
            <button className="secondary" type="submit" disabled={!newTopic.trim()}>
              Create page
            </button>
          </form>
        </aside>

        <section className="editor-area">
          {selected ? (
            <>
              <div className="document-heading">
                <div>
                  <p className="eyebrow">TOPIC</p>
                  <h2>{selected.topic}</h2>
                </div>
                {dirty && <span className="unsaved">Unsaved</span>}
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

              <div className="split-editor">
                <section className="pane">
                  <h3>Markdown</h3>
                  <textarea
                    aria-label={`Edit ${selected.topic}`}
                    value={draft}
                    onChange={(event) => setDraft(event.currentTarget.value)}
                    spellCheck
                  />
                </section>
                <section className="pane preview">
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
            </div>
          )}
        </section>
      </div>

      <footer className="statusbar">{status}</footer>
    </main>
  );
}

export default App;
