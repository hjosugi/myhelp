// UI localization for the desktop editor.
//
// `en` is the source catalog: every key exists there, and its placeholders
// (`{name}`) define the parameters a message accepts. Other catalogs may be
// partial; a missing key falls back to English, then to the key itself, so an
// untranslated string is never blank. See docs/localization.md.

export const SUPPORTED_LOCALES = ["en", "ja"] as const;
export type Locale = (typeof SUPPORTED_LOCALES)[number];
export type LocalePreference = "system" | Locale;

/** Language names are shown in their own language, never translated. */
export const LOCALE_NAMES: Record<Locale, string> = {
  en: "English",
  ja: "日本語",
};

export const LOCALE_STORAGE_KEY = "myhelp.uiLocale";

export const en = {
  "app.eyebrow": "LOCAL-FIRST HELP VAULT",
  "app.skipToEditor": "Skip to editor",
  "language.label": "Language",
  "language.system": "System default",
  "vault.resolving": "Resolving vault…",
  "vault.choose": "Choose vault",
  "vault.chooseAnother": "Choose another vault",
  "save.saving": "Saving…",
  "save.resolveConflict": "Resolve conflict",
  "save.saveChanges": "Save changes",
  "save.saved": "Saved",
  "sidebar.label": "Vault pages",
  "search.label": "Search pages",
  "search.placeholder": "Python, Nix, Git…",
  "search.submit": "Search",
  "pages.label": "Help pages",
  "pages.loading": "Loading pages…",
  "pages.open": "Open {topic}",
  "pages.noMatches": "No pages match this search.",
  "pages.empty": "No pages yet. Create one below.",
  "newPage.label": "New page",
  "newPage.placeholder": "python/new-project",
  "newPage.submit": "Create page",
  "vaultError.eyebrow": "VAULT UNAVAILABLE",
  "vaultError.title": "MyHelp could not open this vault.",
  "vaultError.retry": "Retry",
  "document.eyebrow": "TOPIC",
  "document.unsaved": "Unsaved",
  "document.rename": "Rename",
  "document.delete": "Delete",
  "conflict.eyebrow": "EXTERNAL CHANGE",
  "conflict.changedTitle": "This page changed on disk.",
  "conflict.deletedTitle": "This page was deleted on disk.",
  "conflict.body":
    "The disk version was not overwritten. Your current draft remains in the editor.",
  "conflict.bodyWithDraft":
    "The disk version was not overwritten. Your current draft remains in the editor and is also saved at {path}",
  "conflict.reconcile": "Reconcile and save draft",
  "conflict.loadDisk": "Load disk version",
  "conflict.restoreDraft": "Restore draft as page",
  "conflict.acceptDeletion": "Accept deletion",
  "conflict.reviewDisk": "Review the disk version before reconciling",
  "view.label": "Editor view",
  "view.edit": "Editor",
  "view.split": "Split",
  "view.preview": "Preview",
  "view.cycle": "Cycle",
  "pane.markdown": "Markdown",
  "pane.preview": "Preview",
  "editor.label": "Edit {topic}",
  "welcome.eyebrow": "PLAIN MARKDOWN, EVERYWHERE",
  "welcome.title": "Your commands should be one search away.",
  "welcome.body":
    "Create a page from the sidebar, then read the same file from the desktop app, the CLI, or a tldr-compatible client.",
  "welcome.create": "Create your first page",
  "undo.label": "Deleted page recovery",
  "undo.message": "{topic} is in a readable recovery file.",
  "undo.button": "Undo delete",
  "modal.eyebrow": "CONFIRM ACTION",
  "common.cancel": "Cancel",
  "unsaved.title": "Keep your current changes?",
  "unsaved.description":
    "You have work that has not been committed to the current page. Decide what to do before you {action}.",
  "unsaved.saveAndContinue": "Save and continue",
  "unsaved.discardAndContinue": "Discard draft and continue",
  "unsaved.conflictNote":
    "Saving is unavailable until the external change is resolved. You can cancel and use the conflict controls, or discard this draft.",
  "action.open": "open {topic}",
  "action.create": "create {topic}",
  "action.rename": "rename this page to {topic}",
  "action.delete": "move this page to a recovery file",
  "action.chooseVault": "switch vaults",
  "action.close": "close MyHelp",
  "rename.title": "Rename {topic}",
  "rename.description":
    "The Markdown page and its optional metadata sidecar move together. An existing destination is never overwritten.",
  "rename.label": "New topic",
  "rename.submit": "Rename page",
  "delete.title": "Delete {topic}?",
  "delete.description":
    "{topic} will leave the page list, but MyHelp keeps a readable recovery Markdown file so you can undo the deletion.",
  "delete.confirm": "Move to recovery file",
  "discard.title": "Discard the in-memory draft?",
  "discard.description":
    "The draft could not be copied to disk. Loading the disk version now will permanently discard the only remaining in-memory copy.",
  "discard.confirm": "Discard in-memory draft",
  "discard.keepEditing": "Keep editing",
  "status.loading": "Loading your help vault…",
  "status.firstPage": "Create your first help page.",
  "status.pageCount.one": "{count} page",
  "status.pageCount.other": "{count} pages",
  "status.matchCount.one": "{count} matching page",
  "status.matchCount.other": "{count} matching pages",
  "status.openVaultFailed": "Could not open the vault: {reason}",
  "status.watcherWarning": "Vault watcher warning: {detail}",
  "status.editing": "Editing {topic}",
  "status.openFailed": "Could not open {topic}: {reason}",
  "status.saved": "Saved {topic}",
  "status.conflictUnreadable":
    "Conflict detected, but the disk version could not be read: {reason}",
  "status.conflictDraftCopied": "Conflict: disk version kept; draft copied to {path}",
  "status.conflict": "Conflict: {reason}",
  "status.saveFailed": "Could not save {topic}: {reason}",
  "status.waitForSave": "Wait for the current save to finish before changing pages.",
  "status.created": "Created {topic}",
  "status.createFailed": "Could not create {topic}: {reason}",
  "status.vaultCancelled": "Vault selection cancelled.",
  "status.vaultOpened": "Opened vault {path}",
  "status.vaultSwitchFailed": "Could not switch vaults: {reason}",
  "status.closeFailed": "Could not close MyHelp: {reason}",
  "status.pageGone": "The page is no longer available.",
  "status.renamed": "Renamed {from} to {to}",
  "status.renameFailed": "Could not rename {topic}: {reason}",
  "status.deleted": "Moved {topic} to a recovery file. Undo is available.",
  "status.deleteFailed": "Could not delete {topic}: {reason}",
  "status.resolveBeforeSaving": "Resolve the external change before saving.",
  "status.alreadyDeleted":
    "The page was already deleted on disk; choose Restore or accept deletion.",
  "status.restored": "Restored {topic}",
  "status.restoreFailed": "Could not restore {topic}: {reason}",
  "status.refreshFailed": "Could not refresh {topic}: {reason}",
  "status.reloaded": "Reloaded external changes to {topic}",
  "status.removedOutside": "{topic} was removed outside MyHelp",
  "status.draftCopyFailed":
    "External change detected; the draft is still open but could not be copied: {reason}",
  "status.externalChange":
    "External change detected; disk version kept and draft copied to {path}",
  "status.diskAccepted":
    "Disk revision accepted as the save base. Review the draft, then save.",
  "status.loadedDisk": "Loaded the disk version of {topic}",
  "status.acceptedDeletion": "Accepted the external deletion.",
  "status.restoredDraft": "Restored {topic} from the preserved draft",
  "status.restorePageFailed": "Could not restore the page: {reason}",
  "status.searchFailed": "Search failed: {reason}",
  // How a localized error label and the storage layer's English detail are
  // combined. English shows the detail alone because it already reads as a
  // sentence; other locales lead with the translated label.
  "error.format": "{detail}",
  "error.conflict": "The page changed on disk",
  "error.notFound": "The page does not exist",
  "error.alreadyExists": "The page already exists",
  "error.invalidTopic": "The topic name is not valid",
  "error.unsafePath": "The vault contains an unsafe path",
  "error.pageTooLarge": "The page is too large",
  "error.inputTooLarge": "The input is too large",
  "error.invalidData": "The data is not valid",
  "error.storage": "The vault could not be read or written",
  "error.unknown": "Unexpected error",
} as const;

export type MessageKey = keyof typeof en;
export type Catalog = Partial<Record<MessageKey, string>>;
export type Params = Record<string, string | number>;

type PluralKey = Extract<MessageKey, `${string}.other`>;
export type PluralBase = PluralKey extends `${infer Base}.other` ? Base : never;

export const ja: Catalog = {
  "app.eyebrow": "ローカルファーストのヘルプ保管庫",
  "app.skipToEditor": "エディターへ移動",
  "language.label": "表示言語",
  "language.system": "システム設定に従う",
  "vault.resolving": "保管庫を確認しています…",
  "vault.choose": "保管庫を選ぶ",
  "vault.chooseAnother": "別の保管庫を選ぶ",
  "save.saving": "保存しています…",
  "save.resolveConflict": "競合を解決してください",
  "save.saveChanges": "変更を保存",
  "save.saved": "保存済み",
  "sidebar.label": "保管庫のページ",
  "search.label": "ページを検索",
  "search.placeholder": "Python、Nix、Git…",
  "search.submit": "検索",
  "pages.label": "ヘルプページ",
  "pages.loading": "ページを読み込んでいます…",
  "pages.open": "{topic} を開く",
  "pages.noMatches": "検索に一致するページはありません。",
  "pages.empty": "まだページがありません。下の欄から作成できます。",
  "newPage.label": "新しいページ",
  "newPage.placeholder": "python/new-project",
  "newPage.submit": "ページを作成",
  "vaultError.eyebrow": "保管庫を開けません",
  "vaultError.title": "MyHelp はこの保管庫を開けませんでした。",
  "vaultError.retry": "再試行",
  "document.eyebrow": "トピック",
  "document.unsaved": "未保存",
  "document.rename": "名前を変更",
  "document.delete": "削除",
  "conflict.eyebrow": "外部での変更",
  "conflict.changedTitle": "このページはディスク上で変更されました。",
  "conflict.deletedTitle": "このページはディスク上で削除されました。",
  "conflict.body":
    "ディスク上の版は上書きしていません。現在の下書きはエディターに残っています。",
  "conflict.bodyWithDraft":
    "ディスク上の版は上書きしていません。現在の下書きはエディターに残っており、{path} にも保存しました。",
  "conflict.reconcile": "下書きを調整して保存",
  "conflict.loadDisk": "ディスク上の版を読み込む",
  "conflict.restoreDraft": "下書きをページとして復元",
  "conflict.acceptDeletion": "削除を受け入れる",
  "conflict.reviewDisk": "調整する前にディスク上の版を確認する",
  "view.label": "エディターの表示",
  "view.edit": "エディター",
  "view.split": "分割",
  "view.preview": "プレビュー",
  "view.cycle": "切り替え",
  "pane.markdown": "Markdown",
  "pane.preview": "プレビュー",
  "editor.label": "{topic} を編集",
  "welcome.eyebrow": "どこでも読めるプレーンな Markdown",
  "welcome.title": "コマンドは、検索ひとつで見つかるように。",
  "welcome.body":
    "サイドバーからページを作成すると、同じファイルをデスクトップアプリ、CLI、tldr 互換クライアントのどれからでも読めます。",
  "welcome.create": "最初のページを作成",
  "undo.label": "削除したページの復元",
  "undo.message": "{topic} は、読める形式の復元用ファイルに移動しました。",
  "undo.button": "削除を取り消す",
  "modal.eyebrow": "操作の確認",
  "common.cancel": "キャンセル",
  "unsaved.title": "現在の変更を残しますか？",
  "unsaved.description":
    "現在のページにまだ保存していない作業があります。{action}前に、どうするかを選んでください。",
  "unsaved.saveAndContinue": "保存して続ける",
  "unsaved.discardAndContinue": "下書きを破棄して続ける",
  "unsaved.conflictNote":
    "外部での変更を解決するまで保存できません。キャンセルして競合の操作を使うか、この下書きを破棄してください。",
  "action.open": "「{topic}」を開く",
  "action.create": "「{topic}」を作成する",
  "action.rename": "このページの名前を「{topic}」に変更する",
  "action.delete": "このページを復元用ファイルへ移動する",
  "action.chooseVault": "保管庫を切り替える",
  "action.close": "MyHelp を閉じる",
  "rename.title": "「{topic}」の名前を変更",
  "rename.description":
    "Markdown のページと、あればメタデータのサイドカーを一緒に移動します。移動先に既存のファイルがあっても上書きしません。",
  "rename.label": "新しいトピック",
  "rename.submit": "名前を変更",
  "delete.title": "「{topic}」を削除しますか？",
  "delete.description":
    "{topic} はページ一覧から外れますが、MyHelp は読める形式の復元用 Markdown ファイルを残すので、削除を取り消せます。",
  "delete.confirm": "復元用ファイルへ移動",
  "discard.title": "メモリ上の下書きを破棄しますか？",
  "discard.description":
    "下書きをディスクにコピーできませんでした。いまディスク上の版を読み込むと、メモリ上に残っている唯一のコピーが完全に失われます。",
  "discard.confirm": "メモリ上の下書きを破棄",
  "discard.keepEditing": "編集を続ける",
  "status.loading": "ヘルプ保管庫を読み込んでいます…",
  "status.firstPage": "最初のヘルプページを作成しましょう。",
  "status.pageCount.other": "{count} ページ",
  "status.matchCount.other": "一致するページ: {count} 件",
  "status.openVaultFailed": "保管庫を開けませんでした: {reason}",
  "status.watcherWarning": "保管庫の監視に関する警告: {detail}",
  "status.editing": "{topic} を編集中",
  "status.openFailed": "{topic} を開けませんでした: {reason}",
  "status.saved": "{topic} を保存しました",
  "status.conflictUnreadable":
    "競合を検出しましたが、ディスク上の版を読み込めませんでした: {reason}",
  "status.conflictDraftCopied":
    "競合: ディスク上の版を残し、下書きを {path} にコピーしました",
  "status.conflict": "競合: {reason}",
  "status.saveFailed": "{topic} を保存できませんでした: {reason}",
  "status.waitForSave": "ページを切り替える前に、保存が終わるまでお待ちください。",
  "status.created": "{topic} を作成しました",
  "status.createFailed": "{topic} を作成できませんでした: {reason}",
  "status.vaultCancelled": "保管庫の選択をキャンセルしました。",
  "status.vaultOpened": "保管庫 {path} を開きました",
  "status.vaultSwitchFailed": "保管庫を切り替えられませんでした: {reason}",
  "status.closeFailed": "MyHelp を閉じられませんでした: {reason}",
  "status.pageGone": "このページはもう存在しません。",
  "status.renamed": "{from} の名前を {to} に変更しました",
  "status.renameFailed": "{topic} の名前を変更できませんでした: {reason}",
  "status.deleted": "{topic} を復元用ファイルへ移動しました。取り消せます。",
  "status.deleteFailed": "{topic} を削除できませんでした: {reason}",
  "status.resolveBeforeSaving": "保存する前に、外部での変更を解決してください。",
  "status.alreadyDeleted":
    "このページはすでにディスク上で削除されています。復元するか、削除を受け入れてください。",
  "status.restored": "{topic} を復元しました",
  "status.restoreFailed": "{topic} を復元できませんでした: {reason}",
  "status.refreshFailed": "{topic} を再読み込みできませんでした: {reason}",
  "status.reloaded": "{topic} の外部での変更を読み込みました",
  "status.removedOutside": "{topic} は MyHelp の外で削除されました",
  "status.draftCopyFailed":
    "外部での変更を検出しました。下書きは開いたままですが、コピーできませんでした: {reason}",
  "status.externalChange":
    "外部での変更を検出しました。ディスク上の版を残し、下書きを {path} にコピーしました",
  "status.diskAccepted":
    "ディスク上の版を保存の基準にしました。下書きを確認してから保存してください。",
  "status.loadedDisk": "{topic} のディスク上の版を読み込みました",
  "status.acceptedDeletion": "外部での削除を受け入れました。",
  "status.restoredDraft": "残しておいた下書きから {topic} を復元しました",
  "status.restorePageFailed": "ページを復元できませんでした: {reason}",
  "status.searchFailed": "検索に失敗しました: {reason}",
  "error.format": "{label}（{detail}）",
  "error.conflict": "ページがディスク上で変更されています",
  "error.notFound": "ページが存在しません",
  "error.alreadyExists": "ページはすでに存在します",
  "error.invalidTopic": "トピック名が正しくありません",
  "error.unsafePath": "保管庫に安全でないパスがあります",
  "error.pageTooLarge": "ページが大きすぎます",
  "error.inputTooLarge": "入力が大きすぎます",
  "error.invalidData": "データが正しくありません",
  "error.storage": "保管庫を読み書きできませんでした",
  "error.unknown": "予期しないエラー",
};

export const catalogs: Record<Locale, Catalog> = { en, ja };

/**
 * Parameters a translation may use beyond those in the English source. A
 * translation must keep every English placeholder and may add only these.
 */
export const OPTIONAL_PARAMETERS: Partial<Record<MessageKey, readonly string[]>> = {
  "error.format": ["label"],
};

export type CommandErrorLike = { kind: string; message: string };

/** A UI message kept as data so it re-renders when the language changes. */
export type Message = {
  key: MessageKey | PluralBase;
  params?: Params;
  /** Selects `<key>.<plural category>` and fills `{count}`. */
  count?: number;
  /** Rendered into `{reason}` with the current locale. */
  error?: CommandErrorLike;
};

export function isLocale(value: unknown): value is Locale {
  return (SUPPORTED_LOCALES as readonly unknown[]).includes(value);
}

export function isLocalePreference(value: unknown): value is LocalePreference {
  return value === "system" || isLocale(value);
}

/**
 * Picks the UI locale. An explicit preference wins; otherwise the first
 * operating-system language whose primary subtag is supported (so `ja-JP` and
 * `ja` both select Japanese); otherwise English.
 */
export function resolveLocale(
  preference: LocalePreference,
  systemLanguages: readonly string[],
): Locale {
  if (preference !== "system") return preference;
  for (const language of systemLanguages) {
    const primary = language.trim().toLowerCase().split(/[-_]/)[0];
    if (isLocale(primary)) return primary;
  }
  return "en";
}

export function lookup(locale: Locale, key: string): string | undefined {
  return (
    (catalogs[locale] as Record<string, string | undefined>)[key] ??
    (en as Record<string, string | undefined>)[key]
  );
}

/** Fills `{name}` placeholders; unknown placeholders are left visible. */
export function interpolate(template: string, params: Params = {}): string {
  return template.replace(/\{(\w+)\}/g, (placeholder, name: string) =>
    name in params ? String(params[name]) : placeholder,
  );
}

export function placeholders(template: string): string[] {
  return Array.from(template.matchAll(/\{(\w+)\}/g), (match) => match[1]).sort();
}

export function translate(locale: Locale, key: MessageKey, params?: Params): string {
  return interpolate(lookup(locale, key) ?? key, params);
}

export function translatePlural(
  locale: Locale,
  base: PluralBase,
  count: number,
  params: Params = {},
): string {
  const category = new Intl.PluralRules(locale).select(count);
  const template =
    (catalogs[locale] as Record<string, string | undefined>)[`${base}.${category}`] ??
    (catalogs[locale] as Record<string, string | undefined>)[`${base}.other`] ??
    lookup("en", `${base}.${new Intl.PluralRules("en").select(count)}`) ??
    base;
  return interpolate(template, { ...params, count });
}

export function errorReason(locale: Locale, error: CommandErrorLike): string {
  const labelKey = `error.${error.kind}`;
  const label = lookup(locale, labelKey) ?? lookup(locale, "error.unknown") ?? "";
  return interpolate(lookup(locale, "error.format") ?? "{detail}", {
    label,
    detail: error.message,
  });
}

export function render(locale: Locale, message: Message): string {
  const params: Params = { ...message.params };
  if (message.error) params.reason = errorReason(locale, message.error);
  if (message.count !== undefined) {
    return translatePlural(locale, message.key as PluralBase, message.count, params);
  }
  return translate(locale, message.key as MessageKey, params);
}

/**
 * Splits a template into text and named slots so callers can put markup such
 * as `<code>` or `<strong>` around a parameter without concatenating
 * translated fragments.
 */
export function templateParts(
  locale: Locale,
  key: MessageKey,
): Array<{ text: string } | { slot: string }> {
  const template = lookup(locale, key) ?? key;
  const parts: Array<{ text: string } | { slot: string }> = [];
  let last = 0;
  for (const match of template.matchAll(/\{(\w+)\}/g)) {
    const index = match.index ?? 0;
    if (index > last) parts.push({ text: template.slice(last, index) });
    parts.push({ slot: match[1] });
    last = index + match[0].length;
  }
  if (last < template.length) parts.push({ text: template.slice(last) });
  return parts;
}

type PreferenceStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

function defaultStorage(): PreferenceStorage | null {
  try {
    return typeof window === "undefined" ? null : window.localStorage;
  } catch {
    return null;
  }
}

export function loadLocalePreference(
  storage: PreferenceStorage | null = defaultStorage(),
): LocalePreference {
  try {
    const stored = storage?.getItem(LOCALE_STORAGE_KEY);
    return isLocalePreference(stored) ? stored : "system";
  } catch {
    return "system";
  }
}

export function saveLocalePreference(
  preference: LocalePreference,
  storage: PreferenceStorage | null = defaultStorage(),
): void {
  try {
    if (preference === "system") {
      storage?.removeItem(LOCALE_STORAGE_KEY);
    } else {
      storage?.setItem(LOCALE_STORAGE_KEY, preference);
    }
  } catch {
    // A read-only or disabled storage keeps the choice for this session only.
  }
}

export function systemLanguages(): readonly string[] {
  if (typeof navigator === "undefined") return [];
  if (navigator.languages && navigator.languages.length > 0) return navigator.languages;
  return navigator.language ? [navigator.language] : [];
}
