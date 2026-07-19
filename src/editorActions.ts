export type ViewMode = "edit" | "split" | "preview";

export type DeferredAction =
  | { kind: "open"; topic: string }
  | { kind: "create"; topic: string; title: string }
  | { kind: "rename"; newTopic: string }
  | { kind: "delete" }
  | { kind: "chooseVault" }
  | { kind: "close" };

export type ShortcutAction =
  | "focusSearch"
  | "focusNewPage"
  | "save"
  | "cycleView";

export function needsUnsavedResolution(
  hasUnsavedWork: boolean,
  action: DeferredAction,
): boolean {
  const contextReplacingActions: DeferredAction["kind"][] = [
    "open",
    "create",
    "rename",
    "delete",
    "chooseVault",
    "close",
  ];
  return hasUnsavedWork && contextReplacingActions.includes(action.kind);
}

export function nextViewMode(current: ViewMode): ViewMode {
  if (current === "edit") return "split";
  if (current === "split") return "preview";
  return "edit";
}

export function shortcutAction(
  event: Pick<KeyboardEvent, "altKey" | "ctrlKey" | "key" | "metaKey" | "shiftKey">,
): ShortcutAction | null {
  if (event.altKey || (!event.ctrlKey && !event.metaKey)) return null;

  const key = event.key.toLowerCase();
  if (key === "k" && !event.shiftKey) return "focusSearch";
  if (key === "n" && !event.shiftKey) return "focusNewPage";
  if (key === "s" && !event.shiftKey) return "save";
  if (key === "p" && event.shiftKey) return "cycleView";
  return null;
}
