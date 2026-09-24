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
import {
  CommandErrorLike,
  Locale,
  LOCALE_NAMES,
  LocalePreference,
  Message,
  MessageKey,
  Params,
  SUPPORTED_LOCALES,
  errorReason,
  isLocalePreference,
  loadLocalePreference,
  render,
  resolveLocale,
  saveLocalePreference,
  systemLanguages,
  templateParts,
  translate,
} from "./i18n";
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
  eyebrow: string;
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

function actionLabel(locale: Locale, action: DeferredAction): string {
  switch (action.kind) {
    case "open":
      return translate(locale, "action.open", { topic: action.topic });
    case "create":
      return translate(locale, "action.create", { topic: action.topic });
    case "rename":
      return translate(locale, "action.rename", { topic: action.newTopic });
    case "delete":
      return translate(locale, "action.delete");
    case "chooseVault":
      return translate(locale, "action.chooseVault");
    case "close":
      return translate(locale, "action.close");
  }
}

/** Renders a translated template, placing markup around named slots. */
function RichText({
  locale,
  messageKey,
  slots,
}: {
  locale: Locale;
  messageKey: MessageKey;
  slots: Record<string, ReactNode>;
}) {
  return (
    <>
      {templateParts(locale, messageKey).map((part, index) =>
        "text" in part ? (
          <span key={index}>{part.text}</span>
        ) : (
          <span key={index}>{slots[part.slot] ?? `{${part.slot}}`}</span>
        ),
      )}
    </>
  );
}

function statusMessage(
  key: MessageKey,
  params?: Params,
  error?: CommandErrorLike,
): Message {
  return { key, params, error };
}

function Modal({ children, description, eyebrow, onCancel, title }: ModalProps) {
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
        <p className="eyebrow">{eyebrow}</p>
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
  const [status, setStatus] = useState<Message>({ key: "status.loading" });
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<CommandErrorLike | null>(null);
  const [localePreference, setLocalePreference] = useState<LocalePreference>(() =>
    loadLocalePreference(),
  );
  const [languages, setLanguages] = useState<readonly string[]>(() => systemLanguages());
  const locale = resolveLocale(localePreference, languages);
  const t = (key: MessageKey, params?: Params) => translate(locale, key, params);
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
    document.documentElement.lang = locale;
  }, [locale]);

  useEffect(() => {
    const updateLanguages = () => setLanguages(systemLanguages());
    window.addEventListener("languagechange", updateLanguages);
    return () => window.removeEventListener("languagechange", updateLanguages);
  }, []);

  function changeLocalePreference(value: string) {
    if (!isLocalePreference(value)) return;
    setLocalePreference(value);
    saveLocalePreference(value);
  }

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
        setStatus(statusMessage("status.watcherWarning", { detail: event.payload.message }));
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
          ? { key: "status.firstPage" }
          : { key: "status.pageCount", count: initialPages.length },
      );
      if (initialPages[0]) {
        await openPage(initialPages[0].topic);
      }
    } catch (error) {
      const commandError = parseCommandError(error);
      setLoadError(commandError);
      setStatus(statusMessage("status.openVaultFailed", {}, commandError));
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
    setStatus({ key: "status.matchCount", count: nextPages.length });
  }

  async function openPage(topic: string) {
    try {
      const page = await invoke<Page>("read_page", { topic });
      setSelected(page);
      setDraft(page.content);
      setConflict(null);
      setStatus(statusMessage("status.editing", { topic: page.topic }));
      window.requestAnimationFrame(() => editorTextRef.current?.focus());
    } catch (error) {
      setStatus(statusMessage("status.openFailed", { topic }, parseCommandError(error)));
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
      setStatus(statusMessage("status.saved", { topic: saved.topic }));
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
              statusMessage("status.conflictUnreadable", {}, parseCommandError(readError)),
            );
          }
        }
        setConflict({
          disk,
          draftPath: commandError.draftPath ?? null,
        });
        setStatus(
          commandError.draftPath
            ? statusMessage("status.conflictDraftCopied", { path: commandError.draftPath })
            : statusMessage("status.conflict", {}, commandError),
        );
      } else {
        setStatus(statusMessage("status.saveFailed", { topic: page.topic }, commandError));
      }
      return null;
    } finally {
      setSaving(false);
    }
  }

  function requestAction(action: DeferredAction) {
    if (editorRef.current.saving) {
      setStatus(statusMessage("status.waitForSave"));
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
        setStatus(statusMessage("status.created", { topic: page.topic }));
        window.requestAnimationFrame(() => editorTextRef.current?.focus());
      } catch (error) {
        setStatus(
          statusMessage("status.createFailed", { topic: action.topic }, parseCommandError(error)),
        );
      }
      return;
    }
    if (action.kind === "chooseVault") {
      try {
        const path = await invoke<string | null>("choose_vault");
        if (!path) {
          setStatus(statusMessage("status.vaultCancelled"));
          return;
        }
        setVaultPath(path);
        setQuery("");
        setSelected(null);
        setDraft("");
        setConflict(null);
        setLastDeleted(null);
        await initialize();
        setStatus(statusMessage("status.vaultOpened", { path }));
      } catch (error) {
        setStatus(statusMessage("status.vaultSwitchFailed", {}, parseCommandError(error)));
      }
      return;
    }
    if (action.kind === "close") {
      try {
        await invoke("close_window");
      } catch (error) {
        setStatus(statusMessage("status.closeFailed", {}, parseCommandError(error)));
      }
      return;
    }

    const page = basePage ?? editorRef.current.selected;
    if (!page) {
      setStatus(statusMessage("status.pageGone"));
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
        setStatus(statusMessage("status.renamed", { from: page.topic, to: renamed.topic }));
      } catch (error) {
        setStatus(
          statusMessage("status.renameFailed", { topic: page.topic }, parseCommandError(error)),
        );
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
      setStatus(statusMessage("status.deleted", { topic: page.topic }));
    } catch (error) {
      setStatus(
        statusMessage("status.deleteFailed", { topic: page.topic }, parseCommandError(error)),
      );
    }
  }

  async function resolveUnsaved(saveFirst: boolean) {
    if (modal?.kind !== "unsaved") return;
    const action = modal.action;
    const current = editorRef.current;
    let basePage = current.conflict?.disk ?? current.selected;

    if (saveFirst) {
      if (current.conflict) {
        setStatus(statusMessage("status.resolveBeforeSaving"));
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
      setStatus(statusMessage("status.alreadyDeleted"));
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
      setStatus(statusMessage("status.restored", { topic: page.topic }));
    } catch (error) {
      setStatus(
        statusMessage(
          "status.restoreFailed",
          { topic: lastDeleted.topic },
          parseCommandError(error),
        ),
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
        setStatus(
          statusMessage("status.refreshFailed", { topic: before.selected.topic }, commandError),
        );
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
        setStatus(statusMessage("status.reloaded", { topic: disk.topic }));
      } else {
        setSelected(null);
        setDraft("");
        setConflict(null);
        setStatus(statusMessage("status.removedOutside", { topic: latest.selected.topic }));
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
      setStatus(statusMessage("status.draftCopyFailed", {}, parseCommandError(error)));
    }
    setConflict({ disk, draftPath });
    if (draftPath) {
      setStatus(statusMessage("status.externalChange", { path: draftPath }));
    }
    await loadPageList(queryRef.current);
  }

  function useDiskAsSaveBase() {
    if (!conflict?.disk) return;
    setSelected(conflict.disk);
    setConflict(null);
    setStatus(statusMessage("status.diskAccepted"));
    window.requestAnimationFrame(() => editorTextRef.current?.focus());
  }

  function applyDiskVersion() {
    if (!conflict) return;
    if (conflict.disk) {
      setSelected(conflict.disk);
      setDraft(conflict.disk.content);
      setStatus(statusMessage("status.loadedDisk", { topic: conflict.disk.topic }));
    } else {
      setSelected(null);
      setDraft("");
      setStatus(statusMessage("status.acceptedDeletion"));
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
      setStatus(statusMessage("status.restoredDraft", { topic: restored.topic }));
    } catch (error) {
      setStatus(statusMessage("status.restorePageFailed", {}, parseCommandError(error)));
    }
  }

  async function search(event: FormEvent) {
    event.preventDefault();
    try {
      await refreshPages(query);
    } catch (error) {
      setStatus(statusMessage("status.searchFailed", {}, parseCommandError(error)));
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
        {t("app.skipToEditor")}
      </a>
      <header className="topbar">
        <div>
          <p className="eyebrow">{t("app.eyebrow")}</p>
          <h1>MyHelp</h1>
        </div>
        <div className="topbar-actions">
          <span className="vault-path" title={vaultPath}>
            {vaultPath || t("vault.resolving")}
          </span>
          <label className="language-picker">
            <span>{t("language.label")}</span>
            <select
              onChange={(event) => changeLocalePreference(event.currentTarget.value)}
              value={localePreference}
            >
              <option value="system">{t("language.system")}</option>
              {SUPPORTED_LOCALES.map((option) => (
                <option key={option} lang={option} value={option}>
                  {LOCALE_NAMES[option]}
                </option>
              ))}
            </select>
          </label>
          <button
            aria-label={t("vault.chooseAnother")}
            className="secondary compact"
            disabled={loading}
            onClick={() => requestAction({ kind: "chooseVault" })}
            type="button"
          >
            {t("vault.choose")}
          </button>
          <button
            aria-keyshortcuts="Control+S Meta+S"
            className="primary"
            disabled={!dirty || saving || conflict !== null}
            onClick={() => void savePage()}
            type="button"
          >
            {saving
              ? t("save.saving")
              : conflict
                ? t("save.resolveConflict")
                : dirty
                  ? t("save.saveChanges")
                  : t("save.saved")}
          </button>
        </div>
      </header>

      <div className="workspace">
        <aside aria-label={t("sidebar.label")} className="sidebar">
          <form className="search" onSubmit={(event) => void search(event)}>
            <label htmlFor="search-pages">
              {t("search.label")} <kbd>Ctrl/⌘ K</kbd>
            </label>
            <div className="input-row">
              <input
                aria-keyshortcuts="Control+K Meta+K"
                id="search-pages"
                onChange={(event) => setQuery(event.currentTarget.value)}
                placeholder={t("search.placeholder")}
                ref={searchRef}
                value={query}
              />
              <button type="submit">{t("search.submit")}</button>
            </div>
          </form>

          <nav aria-busy={loading} aria-label={t("pages.label")} className="page-list">
            {loading ? (
              <p className="empty">{t("pages.loading")}</p>
            ) : (
              pages.map((page) => (
                <button
                  aria-label={t("pages.open", { topic: page.topic })}
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
                {query.trim() ? t("pages.noMatches") : t("pages.empty")}
              </p>
            )}
          </nav>

          <form className="new-page" onSubmit={createPage}>
            <label htmlFor="new-topic">
              {t("newPage.label")} <kbd>Ctrl/⌘ N</kbd>
            </label>
            <input
              aria-keyshortcuts="Control+N Meta+N"
              id="new-topic"
              onChange={(event) => setNewTopic(event.currentTarget.value)}
              placeholder={t("newPage.placeholder")}
              ref={newTopicRef}
              value={newTopic}
            />
            <button className="secondary" type="submit" disabled={!newTopic.trim()}>
              {t("newPage.submit")}
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
              <p className="eyebrow">{t("vaultError.eyebrow")}</p>
              <h2>{t("vaultError.title")}</h2>
              <p>{errorReason(locale, loadError)}</p>
              <div className="button-row">
                <button className="primary" onClick={() => void initialize()}>
                  {t("vaultError.retry")}
                </button>
                <button
                  className="secondary"
                  onClick={() => requestAction({ kind: "chooseVault" })}
                >
                  {t("vault.chooseAnother")}
                </button>
              </div>
            </div>
          ) : selected ? (
            <>
              <div className="document-heading">
                <div>
                  <p className="eyebrow">{t("document.eyebrow")}</p>
                  <h2>{selected.topic}</h2>
                </div>
                <div className="document-actions">
                  {dirty && <span className="unsaved">{t("document.unsaved")}</span>}
                  <button className="secondary compact" onClick={startRename} type="button">
                    {t("document.rename")}
                  </button>
                  <button
                    className="danger compact"
                    onClick={() => setModal({ kind: "delete" })}
                    type="button"
                  >
                    {t("document.delete")}
                  </button>
                </div>
              </div>

              {conflict && (
                <section className="conflict-banner" role="alert">
                  <div>
                    <p className="eyebrow">{t("conflict.eyebrow")}</p>
                    <h3>
                      {conflict.disk
                        ? t("conflict.changedTitle")
                        : t("conflict.deletedTitle")}
                    </h3>
                    <p>
                      {conflict.draftPath ? (
                        <RichText
                          locale={locale}
                          messageKey="conflict.bodyWithDraft"
                          slots={{ path: <code>{conflict.draftPath}</code> }}
                        />
                      ) : (
                        t("conflict.body")
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
                          {t("conflict.reconcile")}
                        </button>
                        <button
                          className="secondary"
                          onClick={loadDiskVersion}
                          type="button"
                        >
                          {t("conflict.loadDisk")}
                        </button>
                      </>
                    ) : (
                      <>
                        <button
                          className="primary"
                          onClick={() => void restoreDeletedDraft()}
                          type="button"
                        >
                          {t("conflict.restoreDraft")}
                        </button>
                        <button
                          className="secondary"
                          onClick={loadDiskVersion}
                          type="button"
                        >
                          {t("conflict.acceptDeletion")}
                        </button>
                      </>
                    )}
                  </div>
                  {conflict.disk && (
                    <details>
                      <summary>{t("conflict.reviewDisk")}</summary>
                      <pre>{conflict.disk.content}</pre>
                    </details>
                  )}
                </section>
              )}

              <div className="view-switcher" role="group" aria-label={t("view.label")}>
                {(["edit", "split", "preview"] as const).map((mode) => (
                  <button
                    aria-pressed={viewMode === mode}
                    className={viewMode === mode ? "active" : ""}
                    key={mode}
                    onClick={() => setViewMode(mode)}
                    type="button"
                  >
                    {mode === "edit"
                      ? t("view.edit")
                      : mode === "split"
                        ? t("view.split")
                        : t("view.preview")}
                  </button>
                ))}
                <span className="shortcut-hint">
                  {t("view.cycle")} <kbd>Ctrl/⌘ ⇧ P</kbd>
                </span>
              </div>

              <div
                className={`split-editor view-${viewMode}`}
                id="editor-panes"
              >
                <section aria-hidden={!editorVisible} className="pane editor-pane">
                  <h3>{t("pane.markdown")}</h3>
                  <textarea
                    aria-label={t("editor.label", { topic: selected.topic })}
                    ref={editorTextRef}
                    onChange={(event) => setDraft(event.currentTarget.value)}
                    spellCheck
                    tabIndex={editorVisible ? 0 : -1}
                    value={draft}
                  />
                </section>
                <section aria-hidden={!previewVisible} className="pane preview">
                  <h3>{t("pane.preview")}</h3>
                  <article>
                    <MarkdownPreview source={draft} />
                  </article>
                </section>
              </div>
            </>
          ) : (
            <div className="welcome">
              <p className="eyebrow">{t("welcome.eyebrow")}</p>
              <h2>{t("welcome.title")}</h2>
              <p>{t("welcome.body")}</p>
              <button
                className="primary"
                onClick={() => newTopicRef.current?.focus()}
                type="button"
              >
                {t("welcome.create")}
              </button>
            </div>
          )}
        </section>
      </div>

      {lastDeleted && (
        <aside className="undo-banner" aria-label={t("undo.label")}>
          <span>
            <RichText
              locale={locale}
              messageKey="undo.message"
              slots={{ topic: <strong>{lastDeleted.topic}</strong> }}
            />
          </span>
          <button className="secondary compact" onClick={() => void undoDelete()}>
            {t("undo.button")}
          </button>
        </aside>
      )}
      <div aria-atomic="true" aria-live="polite" className="statusbar" role="status">
        {render(locale, status)}
      </div>

      {modal?.kind === "unsaved" && (
        <Modal
          description={t("unsaved.description", {
            action: actionLabel(locale, modal.action),
          })}
          eyebrow={t("modal.eyebrow")}
          onCancel={() => setModal(null)}
          title={t("unsaved.title")}
        >
          <div className="modal-actions">
            <button
              className="primary"
              data-autofocus
              disabled={conflict !== null}
              onClick={() => void resolveUnsaved(true)}
              type="button"
            >
              {t("unsaved.saveAndContinue")}
            </button>
            <button
              className="danger"
              onClick={() => void resolveUnsaved(false)}
              type="button"
            >
              {t("unsaved.discardAndContinue")}
            </button>
            <button className="secondary" onClick={() => setModal(null)} type="button">
              {t("common.cancel")}
            </button>
          </div>
          {conflict && (
            <p className="modal-note">{t("unsaved.conflictNote")}</p>
          )}
        </Modal>
      )}

      {modal?.kind === "rename" && selected && (
        <Modal
          description={t("rename.description")}
          eyebrow={t("modal.eyebrow")}
          onCancel={() => setModal(null)}
          title={t("rename.title", { topic: selected.topic })}
        >
          <form className="modal-form" onSubmit={submitRename}>
            <label htmlFor="rename-topic">{t("rename.label")}</label>
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
                {t("rename.submit")}
              </button>
              <button className="secondary" onClick={() => setModal(null)} type="button">
                {t("common.cancel")}
              </button>
            </div>
          </form>
        </Modal>
      )}

      {modal?.kind === "delete" && selected && (
        <Modal
          description={t("delete.description", { topic: selected.topic })}
          eyebrow={t("modal.eyebrow")}
          onCancel={() => setModal(null)}
          title={t("delete.title", { topic: selected.topic })}
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
              {t("delete.confirm")}
            </button>
            <button className="secondary" onClick={() => setModal(null)} type="button">
              {t("common.cancel")}
            </button>
          </div>
        </Modal>
      )}

      {modal?.kind === "discardConflict" && (
        <Modal
          description={t("discard.description")}
          eyebrow={t("modal.eyebrow")}
          onCancel={() => setModal(null)}
          title={t("discard.title")}
        >
          <div className="modal-actions">
            <button
              className="danger"
              data-autofocus
              onClick={applyDiskVersion}
              type="button"
            >
              {t("discard.confirm")}
            </button>
            <button className="secondary" onClick={() => setModal(null)} type="button">
              {t("discard.keepEditing")}
            </button>
          </div>
        </Modal>
      )}
    </main>
  );
}

export default App;
