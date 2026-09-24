//! Optional, opt-in Git workflow for a vault (ADR 0004).
//!
//! MyHelp drives the user's installed `git` executable directly (no shell) and
//! never stores credentials: authentication, signing, hooks, and remotes stay
//! with Git's own configuration. Plain files remain the source of truth. The
//! module never force-pushes, rebases, resets, or resolves a conflict, and it
//! leaves conflicted files exactly as Git wrote them.

use serde::Serialize;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use thiserror::Error;

/// Local, uncommitted opt-in flag (`git config --local myhelp.sync true`).
pub const SYNC_CONFIG_KEY: &str = "myhelp.sync";

/// Local files MyHelp creates next to pages that must never be published:
/// conflict copies can hold unsaved drafts and recovery files hold deleted
/// pages. They are added to `.git/info/exclude`, which is itself never
/// committed.
pub const LOCAL_ONLY_PATTERNS: [&str; 2] = ["*.page.conflict-*.md", "*.page.deleted-*.md"];

const EXCLUDE_HEADER: &str = "# MyHelp local-only files (conflict copies and recovery files)";

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("the Git executable could not be started: {0}")]
    GitUnavailable(std::io::Error),
    #[error("the vault is not inside a Git work tree: {}", .0.display())]
    NotARepository(PathBuf),
    #[error("Git sync is not enabled for this vault; run `myhelp sync enable` first")]
    NotEnabled,
    #[error("resolve the conflicted files before continuing: {}", display_paths(.0))]
    Conflicted(Vec<String>),
    #[error("a Git {0} is in progress; finish or abort it with Git first")]
    OperationInProgress(SyncOperation),
    #[error("there are no page changes to commit")]
    NothingToCommit,
    #[error(
        "the local and remote histories have diverged; rerun with --merge to create a merge commit"
    )]
    Diverged,
    #[error("the remote rejected the push; pull first (MyHelp never force-pushes)")]
    PushRejected,
    #[error("git {command} failed: {detail}")]
    GitFailed { command: String, detail: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn display_paths(paths: &[String]) -> String {
    paths.join(", ")
}

pub type SyncResult<T> = std::result::Result<T, SyncError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncOperation {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

impl std::fmt::Display for SyncOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Merge => "merge",
            Self::Rebase => "rebase",
            Self::CherryPick => "cherry-pick",
            Self::Revert => "revert",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncChange {
    /// Path relative to the vault root, with `/` separators.
    pub path: String,
    pub kind: SyncChangeKind,
    /// Whether the change is already staged for the next commit.
    pub staged: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub enabled: bool,
    /// Top level of the enclosing work tree, when there is one.
    pub repository: Option<PathBuf>,
    /// `None` on a detached HEAD or an unborn repository without a branch.
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub operation: Option<SyncOperation>,
    /// Changes inside the vault only; the rest of the repository is ignored.
    pub changes: Vec<SyncChange>,
}

impl SyncStatus {
    pub fn conflicted(&self) -> Vec<String> {
        self.changes
            .iter()
            .filter(|change| change.kind == SyncChangeKind::Conflicted)
            .map(|change| change.path.clone())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PullMode {
    /// `git pull --ff-only`: refuse to create a merge.
    #[default]
    FastForwardOnly,
    /// `git pull --no-rebase --no-edit`: merge, leaving conflicts in place.
    Merge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCommit {
    pub commit: String,
    pub staged: Vec<String>,
}

/// Git sync for one vault directory.
#[derive(Clone, Debug)]
pub struct GitSync {
    root: PathBuf,
    git: PathBuf,
}

impl GitSync {
    pub fn new(vault_root: impl Into<PathBuf>) -> Self {
        Self {
            root: vault_root.into(),
            git: PathBuf::from("git"),
        }
    }

    /// Uses a specific Git executable instead of `git` on `PATH`.
    pub fn with_git(mut self, git: impl Into<PathBuf>) -> Self {
        self.git = git.into();
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reports the vault's Git state without changing anything.
    pub fn status(&self) -> SyncResult<SyncStatus> {
        let Some(repository) = self.work_tree()? else {
            return Ok(SyncStatus {
                enabled: false,
                repository: None,
                branch: None,
                upstream: None,
                ahead: 0,
                behind: 0,
                operation: None,
                changes: Vec::new(),
            });
        };
        let prefix = self.prefix()?;
        let output = self.git_checked(
            &[
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=all",
                "--",
                ".",
            ],
            true,
        )?;
        let mut status = parse_porcelain_v2(&output.stdout, &prefix);
        status.enabled = self.enabled()?;
        status.repository = Some(repository);
        status.operation = self.operation()?;
        Ok(status)
    }

    /// Opts this vault in. With `initialize`, creates a repository at the
    /// vault root when the vault is not already inside a work tree.
    pub fn enable(&self, initialize: bool) -> SyncResult<SyncStatus> {
        fs::create_dir_all(&self.root)?;
        if self.work_tree()?.is_none() {
            if !initialize {
                return Err(SyncError::NotARepository(self.root.clone()));
            }
            self.git_checked(&["init", "--quiet"], false)?;
        }
        self.git_checked(&["config", "--local", SYNC_CONFIG_KEY, "true"], false)?;
        self.exclude_local_only_files()?;
        self.status()
    }

    /// Opts this vault out. The repository and its history are untouched.
    pub fn disable(&self) -> SyncResult<()> {
        if self.work_tree()?.is_none() {
            return Err(SyncError::NotARepository(self.root.clone()));
        }
        let output = self.git(&["config", "--local", "--unset", SYNC_CONFIG_KEY], false)?;
        // Exit status 5 means the key was not set, which is already disabled.
        if output.status.success() || output.status.code() == Some(5) {
            Ok(())
        } else {
            Err(git_failed("config --unset", &output))
        }
    }

    /// Commits page changes inside the vault.
    ///
    /// Modified and deleted tracked files are always included. Untracked files
    /// are included only with `include_new`, so a new page is never published
    /// without an explicit decision. Changes outside the vault are ignored.
    ///
    /// The user's commit hooks and signing configuration apply as usual.
    pub fn commit(&self, message: &str, include_new: bool) -> SyncResult<SyncCommit> {
        let status = self.require_ready()?;
        let has_candidates = status
            .changes
            .iter()
            .any(|change| include_new || change.kind != SyncChangeKind::Untracked);
        if !has_candidates {
            // Also avoids `git add --update -- .`, which fails when no file in
            // the vault is tracked yet.
            return Err(SyncError::NothingToCommit);
        }
        let mode = if include_new { "--all" } else { "--update" };
        self.git_checked(&["add", mode, "--", "."], false)?;

        let staged = self.staged_paths()?;
        if staged.is_empty() {
            return Err(SyncError::NothingToCommit);
        }
        let prefix = self.prefix()?;
        // The pathspec limits the commit to the vault even when the vault is a
        // subdirectory of a repository with other staged work.
        self.git_checked(&["commit", "--quiet", "-m", message, "--", "."], false)?;
        let head = self.git_checked(&["rev-parse", "HEAD"], true)?;
        Ok(SyncCommit {
            commit: String::from_utf8_lossy(&head.stdout).trim().to_owned(),
            staged: staged
                .into_iter()
                .map(|path| strip_prefix(&path, &prefix))
                .collect(),
        })
    }

    /// Pulls from the configured upstream. Never rebases.
    pub fn pull(&self, mode: PullMode) -> SyncResult<SyncStatus> {
        self.require_ready()?;
        let arguments: &[&str] = match mode {
            PullMode::FastForwardOnly => &["pull", "--ff-only", "--no-rebase"],
            PullMode::Merge => &["pull", "--no-rebase", "--no-edit"],
        };
        let output = self.git(arguments, false)?;
        if output.status.success() {
            return self.status();
        }
        let status = self.status()?;
        if !status.conflicted().is_empty() {
            return Err(SyncError::Conflicted(status.conflicted()));
        }
        if mode == PullMode::FastForwardOnly && status.behind > 0 && status.ahead > 0 {
            return Err(SyncError::Diverged);
        }
        Err(git_failed("pull", &output))
    }

    /// Pushes the current branch. Never forces.
    pub fn push(&self) -> SyncResult<SyncStatus> {
        self.require_ready()?;
        let output = self.git(&["push"], false)?;
        if output.status.success() {
            return self.status();
        }
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.contains("[rejected]") || detail.contains("non-fast-forward") {
            return Err(SyncError::PushRejected);
        }
        Err(git_failed("push", &output))
    }

    fn require_ready(&self) -> SyncResult<SyncStatus> {
        let status = self.status()?;
        if status.repository.is_none() {
            return Err(SyncError::NotARepository(self.root.clone()));
        }
        if !status.enabled {
            return Err(SyncError::NotEnabled);
        }
        let conflicted = status.conflicted();
        if !conflicted.is_empty() {
            return Err(SyncError::Conflicted(conflicted));
        }
        if let Some(operation) = status.operation {
            return Err(SyncError::OperationInProgress(operation));
        }
        Ok(status)
    }

    fn work_tree(&self) -> SyncResult<Option<PathBuf>> {
        if !self.root.is_dir() {
            return Ok(None);
        }
        let output = self.git(&["rev-parse", "--show-toplevel"], true)?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(PathBuf::from(
            String::from_utf8_lossy(&output.stdout).trim_end_matches(['\r', '\n']),
        )))
    }

    fn prefix(&self) -> SyncResult<String> {
        let output = self.git_checked(&["rev-parse", "--show-prefix"], true)?;
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end_matches(['\r', '\n'])
            .to_owned())
    }

    fn enabled(&self) -> SyncResult<bool> {
        let output = self.git(
            &["config", "--local", "--type=bool", "--get", SYNC_CONFIG_KEY],
            true,
        )?;
        Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true")
    }

    fn operation(&self) -> SyncResult<Option<SyncOperation>> {
        for (marker, operation) in [
            ("MERGE_HEAD", SyncOperation::Merge),
            ("rebase-merge", SyncOperation::Rebase),
            ("rebase-apply", SyncOperation::Rebase),
            ("CHERRY_PICK_HEAD", SyncOperation::CherryPick),
            ("REVERT_HEAD", SyncOperation::Revert),
        ] {
            let output = self.git_checked(&["rev-parse", "--git-path", marker], true)?;
            let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
            let path = if path.is_absolute() {
                path
            } else {
                self.root.join(path)
            };
            if path.exists() {
                return Ok(Some(operation));
            }
        }
        Ok(None)
    }

    fn staged_paths(&self) -> SyncResult<Vec<String>> {
        let output = self.git_checked(
            &[
                "diff",
                "--cached",
                "--name-only",
                "-z",
                "--no-renames",
                "--",
                ".",
            ],
            true,
        )?;
        Ok(output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8_lossy(path).into_owned())
            .collect())
    }

    fn exclude_local_only_files(&self) -> SyncResult<()> {
        let output = self.git_checked(&["rev-parse", "--git-path", "info/exclude"], true)?;
        let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        let path = if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        };
        let existing = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let missing: Vec<&str> = LOCAL_ONLY_PATTERNS
            .into_iter()
            .filter(|pattern| !existing.lines().any(|line| line.trim() == *pattern))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        let mut block = String::new();
        if !existing.is_empty() && !existing.ends_with('\n') {
            block.push('\n');
        }
        if !existing.contains(EXCLUDE_HEADER) {
            block.push_str(EXCLUDE_HEADER);
            block.push('\n');
        }
        for pattern in missing {
            block.push_str(pattern);
            block.push('\n');
        }
        file.write_all(block.as_bytes())?;
        Ok(())
    }

    fn git_checked(&self, arguments: &[&str], read_only: bool) -> SyncResult<Output> {
        let output = self.git(arguments, read_only)?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(git_failed(&arguments.join(" "), &output))
        }
    }

    fn git<S: AsRef<OsStr>>(&self, arguments: &[S], read_only: bool) -> SyncResult<Output> {
        let mut command = Command::new(&self.git);
        command
            .arg("-C")
            .arg(&self.root)
            .args(arguments)
            .stdin(Stdio::null())
            // Porcelain output is locale-independent, but error text is parsed
            // for rejected pushes, so keep Git's own messages in English.
            .env("LC_ALL", "C")
            .env("LANGUAGE", "C")
            // MyHelp never collects credentials. Git must authenticate through
            // a credential helper or SSH agent instead of prompting on a
            // terminal that the desktop app does not have.
            .env("GIT_TERMINAL_PROMPT", "0");
        if read_only {
            // Status must not take the index lock that a concurrent Git
            // command or editor integration may need.
            command.env("GIT_OPTIONAL_LOCKS", "0");
        }
        command.output().map_err(SyncError::GitUnavailable)
    }
}

fn git_failed(command: &str, output: &Output) -> SyncError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    SyncError::GitFailed {
        command: command.to_owned(),
        detail: if detail.is_empty() {
            format!("exit status {}", output.status)
        } else {
            detail.to_owned()
        },
    }
}

fn strip_prefix(path: &str, prefix: &str) -> String {
    path.strip_prefix(prefix).unwrap_or(path).to_owned()
}

/// Parses `git status --porcelain=v2 --branch -z` into vault-relative changes.
pub fn parse_porcelain_v2(output: &[u8], prefix: &str) -> SyncStatus {
    let mut status = SyncStatus {
        enabled: false,
        repository: None,
        branch: None,
        upstream: None,
        ahead: 0,
        behind: 0,
        operation: None,
        changes: Vec::new(),
    };
    let mut records = output
        .split(|byte| *byte == 0)
        .map(|record| String::from_utf8_lossy(record).into_owned());
    while let Some(record) = records.next() {
        if let Some(header) = record.strip_prefix("# ") {
            let (name, value) = header.split_once(' ').unwrap_or((header, ""));
            match name {
                "branch.head" if value != "(detached)" => status.branch = Some(value.to_owned()),
                "branch.upstream" => status.upstream = Some(value.to_owned()),
                "branch.ab" => {
                    for part in value.split_whitespace() {
                        if let Some(ahead) = part.strip_prefix('+') {
                            status.ahead = ahead.parse().unwrap_or(0);
                        } else if let Some(behind) = part.strip_prefix('-') {
                            status.behind = behind.parse().unwrap_or(0);
                        }
                    }
                }
                _ => {}
            }
            continue;
        }
        let mut fields = record.splitn(2, ' ');
        let kind = fields.next().unwrap_or_default();
        let rest = fields.next().unwrap_or_default();
        let change = match kind {
            "1" => {
                let parts: Vec<&str> = rest.splitn(8, ' ').collect();
                parts.get(7).map(|path| ordinary_change(parts[0], path))
            }
            "2" => {
                let parts: Vec<&str> = rest.splitn(9, ' ').collect();
                // With -z the original path is the next NUL-separated record.
                records.next();
                parts.get(8).map(|path| SyncChange {
                    path: (*path).to_owned(),
                    kind: SyncChangeKind::Renamed,
                    staged: parts[0].as_bytes().first() != Some(&b'.'),
                })
            }
            "u" => {
                let parts: Vec<&str> = rest.splitn(10, ' ').collect();
                parts.get(9).map(|path| SyncChange {
                    path: (*path).to_owned(),
                    kind: SyncChangeKind::Conflicted,
                    staged: false,
                })
            }
            "?" => Some(SyncChange {
                path: rest.to_owned(),
                kind: SyncChangeKind::Untracked,
                staged: false,
            }),
            _ => None,
        };
        if let Some(mut change) = change {
            change.path = strip_prefix(&change.path, prefix);
            status.changes.push(change);
        }
    }
    status
}

fn ordinary_change(xy: &str, path: &str) -> SyncChange {
    let bytes = xy.as_bytes();
    let (index, work_tree) = (
        bytes.first().copied().unwrap_or(b'.'),
        bytes.get(1).copied().unwrap_or(b'.'),
    );
    let code = if index != b'.' { index } else { work_tree };
    SyncChange {
        path: path.to_owned(),
        kind: match code {
            b'A' => SyncChangeKind::Added,
            b'D' => SyncChangeKind::Deleted,
            b'R' | b'C' => SyncChangeKind::Renamed,
            _ => SyncChangeKind::Modified,
        },
        staged: index != b'.',
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GitSync, LOCAL_ONLY_PATTERNS, PullMode, SyncChange, SyncChangeKind, SyncError,
        parse_porcelain_v2,
    };
    use crate::Vault;
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    fn git(directory: &Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(arguments)
            .env("LC_ALL", "C")
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn identify(repository: &Path) {
        git(repository, &["config", "user.name", "MyHelp Test"]);
        git(
            repository,
            &["config", "user.email", "myhelp@example.invalid"],
        );
        git(repository, &["config", "commit.gpgsign", "false"]);
    }

    /// A bare "remote" plus two clones, all in one temporary directory.
    struct Remote {
        _directory: tempfile::TempDir,
        bare: std::path::PathBuf,
        first: std::path::PathBuf,
        second: std::path::PathBuf,
    }

    fn remote_with_two_clones() -> Remote {
        let directory = tempfile::tempdir().expect("temporary directory");
        let bare = directory.path().join("remote.git");
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        git(
            directory.path(),
            &["init", "--quiet", "--bare", "remote.git"],
        );

        fs::create_dir_all(&first).expect("first vault");
        let sync = GitSync::new(&first);
        sync.enable(true).expect("enable first");
        identify(&first);
        fs::write(
            first.join("git.page.md"),
            "# Git\n\n- Status:\n\n`git status`\n",
        )
        .expect("page");
        sync.commit("Add git page", true).expect("initial commit");
        let branch = git(&first, &["branch", "--show-current"]);
        git(
            &first,
            &[
                "remote",
                "add",
                "origin",
                bare.to_str().expect("UTF-8 path"),
            ],
        );
        git(&first, &["push", "--quiet", "-u", "origin", &branch]);

        git(
            directory.path(),
            &[
                "clone",
                "--quiet",
                bare.to_str().expect("UTF-8 path"),
                "second",
            ],
        );
        identify(&second);
        GitSync::new(&second).enable(false).expect("enable second");
        Remote {
            _directory: directory,
            bare,
            first,
            second,
        }
    }

    #[test]
    fn a_plain_folder_is_not_a_repository_and_is_not_touched() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sync = GitSync::new(directory.path());
        let status = sync.status().expect("status");
        assert!(!status.enabled);
        assert!(status.repository.is_none());
        assert!(matches!(
            sync.enable(false),
            Err(SyncError::NotARepository(_))
        ));
        assert!(matches!(
            sync.commit("message", true),
            Err(SyncError::NotARepository(_))
        ));
        assert!(!directory.path().join(".git").exists());
    }

    #[test]
    fn an_existing_repository_stays_opted_out_until_enabled() {
        let directory = tempfile::tempdir().expect("temporary directory");
        git(directory.path(), &["init", "--quiet"]);
        identify(directory.path());
        fs::write(directory.path().join("git.page.md"), "# Git\n").expect("page");

        let sync = GitSync::new(directory.path());
        let status = sync.status().expect("status");
        assert!(!status.enabled);
        assert!(status.repository.is_some());
        assert!(matches!(sync.commit("m", true), Err(SyncError::NotEnabled)));
        assert!(matches!(sync.push(), Err(SyncError::NotEnabled)));
        assert!(matches!(
            sync.pull(PullMode::FastForwardOnly),
            Err(SyncError::NotEnabled)
        ));

        assert!(sync.enable(false).expect("enable").enabled);
        sync.disable().expect("disable");
        assert!(!sync.status().expect("status").enabled);
        sync.disable().expect("disabling twice is harmless");
    }

    #[test]
    fn enabling_excludes_local_only_files_without_committing_the_rule() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sync = GitSync::new(directory.path());
        sync.enable(true).expect("enable");
        sync.enable(true)
            .expect("enabling twice keeps one rule each");
        let exclude =
            fs::read_to_string(directory.path().join(".git/info/exclude")).expect("exclude file");
        for pattern in LOCAL_ONLY_PATTERNS {
            assert_eq!(exclude.lines().filter(|line| *line == pattern).count(), 1);
        }
        assert!(!directory.path().join(".gitignore").exists());
    }

    #[test]
    fn commit_publishes_new_pages_only_on_request_and_never_local_copies() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sync = GitSync::new(directory.path());
        sync.enable(true).expect("enable");
        identify(directory.path());
        let vault = Vault::new(directory.path());
        vault.create("git", Some("Git")).expect("create page");
        let conflict_copy = format!("git.page.conflict-{}.md", "a".repeat(64));
        fs::write(directory.path().join(&conflict_copy), "unsaved draft").expect("copy");

        let status = sync.status().expect("status");
        assert_eq!(
            status.changes,
            [SyncChange {
                path: "git.page.md".to_owned(),
                kind: SyncChangeKind::Untracked,
                staged: false,
            }]
        );
        assert!(matches!(
            sync.commit("Add pages", false),
            Err(SyncError::NothingToCommit)
        ));

        let commit = sync.commit("Add pages", true).expect("commit new page");
        assert_eq!(commit.staged, ["git.page.md"]);
        assert_eq!(commit.commit.len(), 40);
        let tracked = git(directory.path(), &["ls-files"]);
        assert_eq!(tracked, "git.page.md");
        assert!(sync.status().expect("status").changes.is_empty());

        // Tracked edits are committed without --include-new; new pages wait.
        fs::write(directory.path().join("git.page.md"), "# Git\n\nedited\n").expect("edit");
        vault.create("rust", Some("Rust")).expect("second page");
        let commit = sync.commit("Edit git", false).expect("commit edit");
        assert_eq!(commit.staged, ["git.page.md"]);
        assert_eq!(
            sync.status().expect("status").changes[0].path,
            "rust.page.md"
        );
    }

    #[test]
    fn a_vault_inside_a_larger_repository_only_commits_its_own_files() {
        let directory = tempfile::tempdir().expect("temporary directory");
        git(directory.path(), &["init", "--quiet"]);
        identify(directory.path());
        let vault_root = directory.path().join("notes/help");
        fs::create_dir_all(&vault_root).expect("vault");
        fs::write(directory.path().join("README.md"), "unrelated\n").expect("outside file");
        git(directory.path(), &["add", "README.md"]);
        fs::write(vault_root.join("git.page.md"), "# Git\n").expect("page");

        let sync = GitSync::new(&vault_root);
        sync.enable(false).expect("enable");
        let status = sync.status().expect("status");
        assert_eq!(status.changes.len(), 1);
        assert_eq!(status.changes[0].path, "git.page.md");

        let commit = sync.commit("Add help", true).expect("commit");
        assert_eq!(commit.staged, ["git.page.md"]);
        assert_eq!(
            git(
                directory.path(),
                &["show", "--name-only", "--format=", "HEAD"]
            ),
            "notes/help/git.page.md"
        );
        // The unrelated staged file stays staged and uncommitted.
        assert_eq!(
            git(directory.path(), &["diff", "--cached", "--name-only"]),
            "README.md"
        );
    }

    #[test]
    fn pull_and_push_round_trip_through_a_local_remote() {
        let remote = remote_with_two_clones();
        let second = GitSync::new(&remote.second);
        fs::write(remote.second.join("rust.page.md"), "# Rust\n").expect("page");
        second.commit("Add rust", true).expect("commit");
        let status = second.push().expect("push");
        assert_eq!((status.ahead, status.behind), (0, 0));

        let first = GitSync::new(&remote.first);
        let status = first.pull(PullMode::FastForwardOnly).expect("pull");
        assert_eq!((status.ahead, status.behind), (0, 0));
        assert!(remote.first.join("rust.page.md").is_file());
    }

    #[test]
    fn diverged_history_is_never_rebased_or_forced() {
        let remote = remote_with_two_clones();
        let first = GitSync::new(&remote.first);
        let second = GitSync::new(&remote.second);
        fs::write(remote.first.join("git.page.md"), "# Git\n\nfirst clone\n").expect("edit");
        first.commit("First edit", false).expect("commit first");
        first.push().expect("push first");
        let remote_head = git(&remote.bare, &["rev-parse", "HEAD"]);

        fs::write(remote.second.join("git.page.md"), "# Git\n\nsecond clone\n").expect("edit");
        second.commit("Second edit", false).expect("commit second");
        let local_head = git(&remote.second, &["rev-parse", "HEAD"]);

        assert!(matches!(second.push(), Err(SyncError::PushRejected)));
        assert_eq!(git(&remote.bare, &["rev-parse", "HEAD"]), remote_head);

        assert!(matches!(
            second.pull(PullMode::FastForwardOnly),
            Err(SyncError::Diverged)
        ));
        // Neither a rebase nor a reset happened.
        assert_eq!(git(&remote.second, &["rev-parse", "HEAD"]), local_head);

        let error = second.pull(PullMode::Merge).expect_err("conflicting merge");
        let SyncError::Conflicted(paths) = error else {
            panic!("expected a conflict, got {error:?}");
        };
        assert_eq!(paths, ["git.page.md"]);

        // The conflicted page is left exactly as Git wrote it: a plain,
        // readable file with both versions and the usual markers.
        let content = fs::read_to_string(remote.second.join("git.page.md")).expect("readable");
        assert!(content.contains("<<<<<<<"));
        assert!(content.contains("first clone"));
        assert!(content.contains("second clone"));
        assert_eq!(
            Vault::new(&remote.second)
                .read("git")
                .expect("vault read")
                .content,
            content
        );

        let status = second.status().expect("status");
        assert_eq!(status.conflicted(), ["git.page.md"]);
        assert_eq!(status.operation, Some(super::SyncOperation::Merge));
        assert!(matches!(
            second.commit("m", false),
            Err(SyncError::Conflicted(_))
        ));
        assert!(matches!(second.push(), Err(SyncError::Conflicted(_))));
        assert_eq!(git(&remote.bare, &["rev-parse", "HEAD"]), remote_head);
    }

    #[test]
    fn parses_porcelain_v2_records_relative_to_the_vault() {
        let output = [
            "# branch.oid 0123456789abcdef0123456789abcdef01234567",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +2 -1",
            "1 .M N... 100644 100644 100644 aaa bbb help/git.page.md",
            "1 A. N... 000000 100644 100644 000 ccc help/new.page.md",
            "2 R. N... 100644 100644 100644 ddd ddd R100 help/renamed.page.md",
            "help/old.page.md",
            "u UU N... 100644 100644 100644 100644 e1 e2 e3 help/both.page.md",
            "? help/draft.page.md",
            "",
        ]
        .join("\0");
        let status = parse_porcelain_v2(output.as_bytes(), "help/");
        assert_eq!(status.branch.as_deref(), Some("main"));
        assert_eq!(status.upstream.as_deref(), Some("origin/main"));
        assert_eq!((status.ahead, status.behind), (2, 1));
        let summary: Vec<(&str, SyncChangeKind, bool)> = status
            .changes
            .iter()
            .map(|change| (change.path.as_str(), change.kind, change.staged))
            .collect();
        assert_eq!(
            summary,
            [
                ("git.page.md", SyncChangeKind::Modified, false),
                ("new.page.md", SyncChangeKind::Added, true),
                ("renamed.page.md", SyncChangeKind::Renamed, true),
                ("both.page.md", SyncChangeKind::Conflicted, false),
                ("draft.page.md", SyncChangeKind::Untracked, false),
            ]
        );

        let detached = parse_porcelain_v2(b"# branch.head (detached)\0", "");
        assert_eq!(detached.branch, None);
    }
}
