import { FormEvent, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import "./App.css";

type PageSummary = {
  topic: string;
  title: string;
  path: string;
};

type Page = PageSummary & {
  content: string;
};

function App() {
  const [pages, setPages] = useState<PageSummary[]>([]);
  const [selected, setSelected] = useState<Page | null>(null);
  const [draft, setDraft] = useState("");
  const [query, setQuery] = useState("");
  const [newTopic, setNewTopic] = useState("");
  const [vaultPath, setVaultPath] = useState("");
  const [status, setStatus] = useState("Loading your help vault…");
  const [saving, setSaving] = useState(false);

  const dirty = selected !== null && draft !== selected.content;

  useEffect(() => {
    void initialize();
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
      setStatus(`Could not open the vault: ${String(error)}`);
    }
  }

  async function refreshPages(search = query) {
    const nextPages = search.trim()
      ? await invoke<PageSummary[]>("search_pages", { query: search.trim() })
      : await invoke<PageSummary[]>("list_pages");
    setPages(nextPages);
    setStatus(`${nextPages.length} matching page${nextPages.length === 1 ? "" : "s"}`);
  }

  async function openPage(topic: string) {
    if (dirty && !window.confirm("Discard the unsaved changes?")) {
      return;
    }

    try {
      const page = await invoke<Page>("read_page", { topic });
      setSelected(page);
      setDraft(page.content);
      setStatus(`Editing ${page.topic}`);
    } catch (error) {
      setStatus(`Could not open ${topic}: ${String(error)}`);
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
      await refreshPages("");
      setSelected(page);
      setDraft(page.content);
      setStatus(`Created ${page.topic}`);
    } catch (error) {
      setStatus(`Could not create ${topic}: ${String(error)}`);
    }
  }

  async function savePage() {
    if (!selected) return;

    setSaving(true);
    try {
      const page = await invoke<Page>("save_page", {
        topic: selected.topic,
        content: draft,
      });
      setSelected(page);
      await refreshPages();
      setStatus(`Saved ${page.topic}`);
    } catch (error) {
      setStatus(`Could not save ${selected.topic}: ${String(error)}`);
    } finally {
      setSaving(false);
    }
  }

  async function search(event: FormEvent) {
    event.preventDefault();
    try {
      await refreshPages(query);
    } catch (error) {
      setStatus(`Search failed: ${String(error)}`);
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
            disabled={!dirty || saving}
            onClick={() => void savePage()}
          >
            {saving ? "Saving…" : dirty ? "Save changes" : "Saved"}
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
                    <ReactMarkdown>{draft}</ReactMarkdown>
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
