# ADR 0004: Opt-in Git sync through the installed Git executable

- Status: Accepted
- Date: 2026-09-25
- Issue: [#9](https://github.com/hjosugi/myhelp/issues/9)
- Depends on: [#1](https://github.com/hjosugi/myhelp/issues/1) (revision
  checks and conflict copies), [ADR 0001](0001-page-metadata-sidecars.md)
  (metadata sidecars)

## Context

A vault is a folder of plain Markdown. Many users already keep it in Git,
Syncthing, Dropbox, or no sync at all, and MyHelp must keep working the same
way for all of them. Users who choose Git want to see what changed and to
commit, pull, and push without leaving MyHelp, but they also expect MyHelp to:

- never hold or ask for their credentials;
- never rewrite history, force-push, or resolve a conflict for them;
- never publish drafts, recovery files, or pages they did not choose to share;
- leave every file, including a conflicted one, readable with any editor.

Three ways to drive Git were compared (checked 2026-09-25):

<!-- markdownlint-disable MD013 MD060 -->

| Option | Credentials | Push, merge, hooks | Other costs | Verdict |
|---|---|---|---|---|
| Manual Git only | The user's Git setup | Everything Git does | No in-app status or conflict view | Always supported; not sufficient alone |
| `git2` (libgit2 bindings) | The application must implement credential callbacks for HTTPS, SSH agents, and keys | Push and merge exist; client hooks are not run | C library plus OpenSSL/libssh2 on Linux in both workspaces; behavior differs from the user's Git in config, hooks, and signing | Rejected |
| `gix` (gitoxide) | Can run `git credential` helpers | Push, merge orchestration, `MERGE_HEAD` state, and hooks are unchecked in the project's [crate status](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md) | Pure Rust, but incomplete for this workflow | Rejected for now; revisit when push and merge state land |
| Installed `git` executable | Git's own credential helpers and SSH agent; MyHelp sees nothing | Everything, with the user's config, hooks, and signing | Requires Git on `PATH`; porcelain output must be parsed | **Chosen** |

<!-- markdownlint-enable MD013 MD060 -->

## Decision

`myhelp-core` owns a small `sync` module that runs the installed `git`
executable directly (never through a shell) with `-C <vault>`. The CLI exposes
it as `myhelp sync status|enable|disable|commit|pull|push`; the desktop app will
call the same module through typed Tauri commands.

### Opt-in per vault

- Nothing runs Git on a vault that has not opted in, except the read-only
  `status`.
- `sync enable` records the opt-in as `myhelp.sync = true` in the repository's
  **local** Git config (`.git/config`). The flag is never committed, so cloning
  a vault never opts a new machine in.
- `sync enable --init` creates a repository at the vault root when there is
  none. Without `--init`, a vault outside a work tree stays untouched.
- `sync disable` removes only the flag. History, remotes, and files remain.
- A vault may be a subdirectory of a larger repository (for example a dotfiles
  repository). Status and commits are limited to the vault's pathspec; pull and
  push act on the repository, as they would from the command line.

### Credential boundary

- MyHelp never reads, stores, or prompts for credentials. Git authenticates
  through the user's credential helper or SSH agent.
- Every Git process runs with `GIT_TERMINAL_PROMPT=0` and a closed stdin, so an
  unconfigured remote fails with Git's message instead of waiting on a prompt
  the desktop app could not show.
- Remotes, URLs, and tokens are whatever the repository already has. MyHelp does
  not add, print, or rewrite them.

### Privacy boundary

- `sync enable` adds MyHelp's local-only files to `.git/info/exclude`, which is
  never committed: conflict copies (`*.page.conflict-*.md`) can hold unsaved
  drafts, and recovery files (`*.page.deleted-*.md`) hold deleted pages.
- `sync commit` stages modified and deleted **tracked** files only. A page Git
  does not track yet is committed only with `--include-new`, and `status` lists
  such pages as new, so publishing a page is always an explicit decision.
- Imported third-party pages (tldr imports with a provenance sidecar) and
  private pages follow the same rule: they stay local until the user includes
  them. Pages that must never leave the machine belong in `.gitignore` or
  `.git/info/exclude`; MyHelp respects both because Git applies them.
- Commit hooks and signing configured by the user run as usual.

### No destructive operations

- `pull` is `git pull --ff-only --no-rebase` by default. When histories have
  diverged it stops and reports it; `pull --merge` creates a merge commit
  (`--no-rebase --no-edit`). MyHelp never rebases, resets, stashes, or checks
  out over local changes.
- `push` is a plain `git push`. MyHelp has no force option of any kind,
  including `--force-with-lease`; a rejected push is reported with the advice
  to pull first.
- `commit`, `pull`, and `push` refuse to run while a merge, rebase,
  cherry-pick, or revert is in progress or while any vault file is conflicted.

### Conflict UX

- A conflicting merge leaves each file exactly as Git wrote it: plain text with
  `<<<<<<<`/`>>>>>>>` markers, readable by MyHelp, any editor, or tldr clients.
- `status` lists conflicted paths separately from other changes and reports the
  operation in progress. MyHelp never picks a side.
- The user resolves the file (in MyHelp's editor or elsewhere) and concludes
  the merge with Git (`git add` and `git commit`, or `git merge --abort`).
  MyHelp's own revision checks still prevent a save from silently overwriting a
  file that Git changed underneath the editor.
- Desktop presentation (planned): a sync strip showing branch, ahead/behind,
  and the number of new, changed, and conflicted pages; a conflict list whose
  entries open the page with a banner explaining the markers; and the same
  explicit Commit / Pull / Push buttons with no automatic background sync.

### Users who do not use Git

Syncthing, Dropbox, and plain-folder vaults need no configuration and see no
Git UI. MyHelp's conflict copies, revision checks, and recovery files remain the
protection against concurrent edits from any sync tool.

## Consequences

- Git must be installed for the sync commands; everything else works without
  it. A missing executable is reported as such.
- Behavior matches the user's Git exactly: configuration, hooks, signing,
  credential helpers, and ignore rules.
- Output parsing uses `git status --porcelain=v2 -z`, which Git keeps stable
  across versions and locales. Git's English stderr is requested with
  `LC_ALL=C` only to recognize a rejected push.
- Tests create temporary repositories, including a local bare remote and two
  clones for divergence and conflict cases. No network access is needed.
