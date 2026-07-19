# Desktop accessibility contract

Status: implemented and automated for the pre-1.0 desktop preview on
2026-07-19. Manual assistive-technology checks remain required before a stable
release.

MyHelp targets WCAG 2.2 Level AA for its React editor. This is an engineering
target, not a certification claim. The desktop app also follows native platform
keyboard conventions where the web content and system window meet.

## Keyboard workflow

<!-- markdownlint-disable MD013 MD060 -->

| Action | Windows/Linux | macOS | Result |
|---|---|---|---|
| Search pages | `Ctrl+K` | `Cmd+K` | Focuses and selects the search field |
| New page | `Ctrl+N` | `Cmd+N` | Focuses and selects the topic field |
| Save | `Ctrl+S` | `Cmd+S` | Saves the current dirty draft |
| Cycle editor view | `Ctrl+Shift+P` | `Cmd+Shift+P` | Cycles Editor, Split, and Preview |
| Move through controls | `Tab` / `Shift+Tab` | same | Follows visual reading order |
| Cancel a confirmation | `Escape` | same | Closes the modal and restores prior focus |

<!-- markdownlint-enable MD013 MD060 -->

All primary workflow controls are native buttons, inputs, or textareas. The
Editor/Split/Preview selector exposes pressed state as a labelled button group.
The page list identifies the current page, and the skip link moves directly to
the editor workspace.

Confirmation dialogs move focus to the primary decision, trap `Tab` within the
dialog, hide and inert the editor behind the modal, close on `Escape`, and
restore focus when possible. No single printable-character shortcut is
registered, so normal typing and assistive-technology character commands remain
available.

## Data-loss decisions

One decision path covers every action that can replace editor context:

<!-- markdownlint-disable MD013 MD060 -->

| Action | Dirty behavior | Recovery |
|---|---|---|
| Open or create a page | Save and continue, discard and continue, or cancel | Current page remains selected on cancel |
| Rename | Same dirty decision after the new topic is entered | Existing destinations are never replaced |
| Delete | Explicit deletion explanation, then the dirty decision | Page moves to readable recovery Markdown and exposes Undo |
| Switch vault | Same dirty decision before the native chooser opens | Cancelling the chooser keeps the current vault |
| Close window | Native close is intercepted and shows the same decision | Cancelling keeps the window and draft open |
| Load an external disk version | Extra warning when the conflict copy failed | In-memory draft remains until explicit discard |

<!-- markdownlint-enable MD013 MD060 -->

Save conflicts never overwrite the disk version. A failed operation keeps its
source file and reports the error through the visible status region.

## Visual system

`src/App.css` defines shared role-based custom properties for:

- UI, display, and monospace font families;
- caption, small, body, lead, title, heading, and display type sizes;
- compact, body, and relaxed line heights;
- spacing, control height, border, radius, focus, and shadow roles;
- semantic light and dark colors for canvas, surface, text, accent, warning,
  danger, code, and overlay states.

Component rules reference these roles instead of introducing one-off font
sizes. A frontend test rejects raw component `font-size` declarations. Another
test calculates WCAG contrast from the light and dark semantic palettes; all
normal-text pairs used by the UI are at least 4.5:1. The lowest tested light
pair is 4.85:1 and the lowest dark pair is 5.79:1.

Every interactive element has a persistent `:focus-visible` outline with an
additional field ring. Controls use a 44 CSS pixel default target height.
Layouts reflow from split panes to a single column, and no editor action depends
on hover, drag, color, or animation alone.

The stylesheet honors `prefers-color-scheme`, `prefers-reduced-motion`, and
forced-colors. Reduced motion removes nonessential transition duration.

## Screen-reader and status behavior

- Search, new-topic, editor, vault, rename, delete, view, and recovery controls
  have explicit or native accessible names.
- Loading regions expose `aria-busy`.
- Conflicts and unrecoverable load failures use alerts.
- Routine save, search, watcher, rename, delete, and restore results update a
  polite, atomic `role="status"` region without moving focus.
- Markdown preview links and images remain inert text under the security model.

The automated React tests run axe-core against the loaded editor. jsdom cannot
calculate rendered color contrast, so axe's contrast rule is disabled there
and the palette test covers the exact token pairs instead.

## Manual release matrix

Run this matrix on representative native packages before calling a release
stable:

<!-- markdownlint-disable MD013 MD060 -->

| Platform | Assistive technology | Checks |
|---|---|---|
| Windows | NVDA and Windows High Contrast | Full keyboard workflow, dialog names/focus, status announcements, 200% scaling |
| macOS | VoiceOver | Full keyboard workflow, native vault chooser, close interception, light/dark appearance |
| Linux | Orca on the supported WebKitGTK stack | Full keyboard workflow, focus visibility, chooser, external-change alert |
| All | Keyboard only | Save/discard/cancel for every context change, delete/Undo, no focus trap |

<!-- markdownlint-enable MD013 MD060 -->

Record the operating-system, webview, and assistive-technology versions in the
release issue. Automated checks do not replace this matrix.

## References

- [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- [WAI Focus Visible guidance](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible)
- [WAI Status Messages guidance](https://www.w3.org/WAI/WCAG22/Understanding/status-messages)
- [Tauri close-request API](https://v2.tauri.app/reference/javascript/api/namespacewindow/#oncloserequested)
- [MDN beforeunload guidance](https://developer.mozilla.org/en-US/docs/Web/API/Window/beforeunload_event)
