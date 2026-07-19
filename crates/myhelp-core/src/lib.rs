use atomic_write_file::AtomicWriteFile;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;
use thiserror::Error;
use walkdir::WalkDir;

const PAGE_SUFFIX: &str = ".page.md";
const CONFLICT_MARKER: &str = ".page.conflict-";
const DELETED_MARKER: &str = ".page.deleted-";
const METADATA_SUFFIX: &str = ".page.meta.yaml";
pub const MAX_PAGE_BYTES: usize = 1024 * 1024;
pub const MAX_SEARCH_QUERY_BYTES: usize = 1024;
pub const MAX_TOPIC_BYTES: usize = 240;

#[derive(Debug, Error)]
pub enum Error {
    #[error("could not determine an operating-system data directory")]
    MissingDataDirectory,
    #[error("invalid topic: {0}")]
    InvalidTopic(String),
    #[error("page already exists: {0}")]
    AlreadyExists(String),
    #[error("page does not exist: {0}")]
    NotFound(String),
    #[error("page changed on disk since it was read: {topic}")]
    Conflict {
        topic: String,
        expected: PageRevision,
        actual: Option<PageRevision>,
    },
    #[error("refusing to follow a symlink or reparse point in the vault: {}", .0.display())]
    UnsafeSymlink(PathBuf),
    #[error("vault page is not a regular file: {}", .0.display())]
    UnsafeFileType(PathBuf),
    #[error("page exceeds the {max_bytes}-byte limit: {}", path.display())]
    PageTooLarge { path: PathBuf, max_bytes: usize },
    #[error("{field} exceeds the {max_bytes}-byte limit")]
    InputTooLarge {
        field: &'static str,
        max_bytes: usize,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    WalkDir(#[from] walkdir::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageSummary {
    pub topic: String,
    pub title: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PageRevision {
    /// Decimal nanoseconds since the Unix epoch, encoded as text for JavaScript safety.
    pub modified_unix_nanos: Option<String>,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Page {
    pub topic: String,
    pub title: String,
    pub content: String,
    pub path: PathBuf,
    pub revision: PageRevision,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeletedPage {
    pub topic: String,
    pub recovery_token: String,
    pub recovery_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

impl Vault {
    pub fn discover() -> Result<Self> {
        if let Some(path) = env::var_os("MYHELP_PAGES_DIR") {
            return Ok(Self::new(path));
        }

        let project_dirs =
            ProjectDirs::from("dev", "myhelp", "myhelp").ok_or(Error::MissingDataDirectory)?;
        Ok(Self::new(project_dirs.data_local_dir().join("pages")))
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn ensure(&self) -> Result<()> {
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                return Err(Error::UnsafeSymlink(self.root.clone()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        fs::create_dir_all(&self.root)?;

        let metadata = fs::symlink_metadata(&self.root)?;
        if is_symlink_or_reparse(&metadata) {
            return Err(Error::UnsafeSymlink(self.root.clone()));
        }
        if !metadata.is_dir() {
            return Err(Error::Io(std::io::Error::new(
                ErrorKind::NotADirectory,
                format!("vault is not a directory: {}", self.root.display()),
            )));
        }
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<PageSummary>> {
        self.ensure()?;
        let mut pages = Vec::new();

        for entry in WalkDir::new(&self.root).follow_links(false) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }

            let relative = entry
                .path()
                .strip_prefix(&self.root)
                .expect("walked entries stay inside the vault");
            let relative_text = relative.to_string_lossy().replace('\\', "/");
            let Some(topic) = relative_text.strip_suffix(PAGE_SUFFIX) else {
                continue;
            };
            self.reject_path_components(entry.path())?;
            let (content, _) = read_snapshot(entry.path())?;
            pages.push(PageSummary {
                topic: topic.to_owned(),
                title: title_from_content(&content, topic),
                path: entry.path().to_path_buf(),
            });
        }

        pages.sort_by(|left, right| left.topic.cmp(&right.topic));
        Ok(pages)
    }

    pub fn read(&self, topic: &str) -> Result<Page> {
        let path = self.page_path(topic)?;
        self.ensure()?;
        self.reject_path_components(&path)?;

        let (content, revision) = match read_snapshot(&path) {
            Ok(snapshot) => snapshot,
            Err(Error::Io(error)) if error.kind() == ErrorKind::NotFound => {
                return Err(Error::NotFound(topic.to_owned()));
            }
            Err(error) => return Err(error),
        };

        Ok(Page {
            topic: topic.to_owned(),
            title: title_from_content(&content, topic),
            content,
            path,
            revision,
        })
    }

    pub fn create(&self, topic: &str, title: Option<&str>) -> Result<Page> {
        let title = title.unwrap_or(topic);
        let content = format!(
            "# {title}\n\n> Personal help for {title}.\n\n- Add an example:\n\n`command --option`\n"
        );
        self.create_with_content(topic, &content)
    }

    pub fn create_with_content(&self, topic: &str, content: &str) -> Result<Page> {
        let path = self.page_path(topic)?;
        reject_oversized_page(&path, content.len() as u64)?;
        self.ensure_parent_directories(&path)?;

        match fs::symlink_metadata(&path) {
            Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                return Err(Error::UnsafeSymlink(path));
            }
            Ok(_) => return Err(Error::AlreadyExists(topic.to_owned())),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let staged = stage_atomic_write(&path, content)?;
        self.reject_path_components(&path)?;
        match fs::symlink_metadata(&path) {
            Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                return Err(Error::UnsafeSymlink(path));
            }
            Ok(_) => return Err(Error::AlreadyExists(topic.to_owned())),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        staged.commit()?;

        self.read(topic)
    }

    pub fn save(
        &self,
        topic: &str,
        content: &str,
        expected_revision: &PageRevision,
    ) -> Result<Page> {
        let path = self.page_path(topic)?;
        reject_oversized_page(&path, content.len() as u64)?;
        self.ensure()?;
        self.reject_path_components(&path)?;

        let actual = self.current_revision(topic)?;
        if actual.as_ref() != Some(expected_revision) {
            return Err(conflict(topic, expected_revision, actual));
        }

        let staged = stage_atomic_write(&path, content)?;

        // Recheck after the complete replacement has been staged. This narrows the
        // compare-and-swap window without ever exposing partially written content.
        self.reject_path_components(&path)?;
        let actual = self.current_revision(topic)?;
        if actual.as_ref() != Some(expected_revision) {
            return Err(conflict(topic, expected_revision, actual));
        }

        staged.commit()?;
        self.read(topic)
    }

    /// Rename a page without replacing an existing destination.
    ///
    /// The page and an optional metadata sidecar use platform no-clobber rename
    /// operations. This keeps the original readable if any move step fails.
    pub fn rename(
        &self,
        topic: &str,
        new_topic: &str,
        expected_revision: &PageRevision,
    ) -> Result<Page> {
        let source = self.page_path(topic)?;
        let destination = self.page_path(new_topic)?;
        self.ensure()?;
        self.reject_path_components(&source)?;
        self.reject_path_components(&destination)?;

        let actual = self.current_revision(topic)?;
        if actual.as_ref() != Some(expected_revision) {
            return Err(conflict(topic, expected_revision, actual));
        }
        if topic == new_topic {
            return self.read(topic);
        }
        self.reject_occupied_destination(new_topic, &destination)?;

        // Recheck immediately before the no-clobber move, which fails if
        // another writer won the destination name.
        let actual = self.current_revision(topic)?;
        if actual.as_ref() != Some(expected_revision) {
            return Err(conflict(topic, expected_revision, actual));
        }
        self.move_with_sidecar_no_replace(&source, &destination)?;
        self.read(new_topic)
    }

    /// Move a page to a readable recovery file rather than permanently deleting it.
    pub fn delete(&self, topic: &str, expected_revision: &PageRevision) -> Result<DeletedPage> {
        let source = self.page_path(topic)?;
        self.ensure()?;
        self.reject_path_components(&source)?;

        let actual = self.current_revision(topic)?;
        if actual.as_ref() != Some(expected_revision) {
            return Err(conflict(topic, expected_revision, actual));
        }

        let digest = &expected_revision.content_sha256;
        let (recovery_token, recovery_path) = self.available_recovery_path(&source, digest)?;

        let actual = self.current_revision(topic)?;
        if actual.as_ref() != Some(expected_revision) {
            return Err(conflict(topic, expected_revision, actual));
        }
        self.move_with_sidecar_no_replace(&source, &recovery_path)?;

        Ok(DeletedPage {
            topic: topic.to_owned(),
            recovery_token,
            recovery_path,
        })
    }

    /// Restore a page previously moved aside by [`Vault::delete`].
    pub fn restore_deleted(&self, topic: &str, recovery_token: &str) -> Result<Page> {
        validate_recovery_token(recovery_token)?;
        let destination = self.page_path(topic)?;
        let source = self.recovery_path(&destination, recovery_token)?;
        self.ensure()?;
        self.reject_path_components(&source)?;
        self.reject_path_components(&destination)?;
        self.reject_occupied_destination(topic, &destination)?;

        match read_snapshot(&source) {
            Ok(_) => {}
            Err(Error::Io(error)) if error.kind() == ErrorKind::NotFound => {
                return Err(Error::NotFound(topic.to_owned()));
            }
            Err(error) => return Err(error),
        }

        self.move_with_sidecar_no_replace(&source, &destination)?;
        self.read(topic)
    }

    /// Preserve a caller-owned draft without replacing the page currently on disk.
    ///
    /// Conflict copies are ordinary Markdown files adjacent to the page, but their
    /// names do not end in `.page.md`, so they do not appear as normal vault pages.
    pub fn preserve_conflict_copy(&self, topic: &str, content: &str) -> Result<PathBuf> {
        let page_path = self.page_path(topic)?;
        reject_oversized_page(&page_path, content.len() as u64)?;
        self.ensure_parent_directories(&page_path)?;

        let digest = content_sha256(content.as_bytes());
        let stem = page_path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(PAGE_SUFFIX))
            .ok_or_else(|| Error::InvalidTopic(topic.to_owned()))?;
        let parent = page_path
            .parent()
            .expect("a page path always has a parent directory");

        for attempt in 0_u16..=u16::MAX {
            let suffix = if attempt == 0 {
                String::new()
            } else {
                format!("-{attempt}")
            };
            let path = parent.join(format!("{stem}{CONFLICT_MARKER}{digest}{suffix}.md"));
            self.reject_path_components(&path)?;

            match read_snapshot(&path) {
                Ok((existing, _)) if existing == content => return Ok(path),
                Ok(_) => continue,
                Err(Error::Io(error)) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }

            let staged = stage_atomic_write(&path, content)?;
            self.reject_path_components(&path)?;
            match fs::symlink_metadata(&path) {
                Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                    return Err(Error::UnsafeSymlink(path));
                }
                Ok(_) => continue,
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            staged.commit()?;
            return Ok(path);
        }

        Err(Error::Io(std::io::Error::new(
            ErrorKind::AlreadyExists,
            "could not allocate a unique conflict-copy name",
        )))
    }

    pub fn search(&self, query: &str) -> Result<Vec<PageSummary>> {
        if query.len() > MAX_SEARCH_QUERY_BYTES {
            return Err(Error::InputTooLarge {
                field: "search query",
                max_bytes: MAX_SEARCH_QUERY_BYTES,
            });
        }
        let query = query.to_lowercase();
        let mut matches = Vec::new();

        for summary in self.list()? {
            let page = self.read(&summary.topic)?;
            if summary.topic.to_lowercase().contains(&query)
                || summary.title.to_lowercase().contains(&query)
                || page.content.to_lowercase().contains(&query)
            {
                matches.push(summary);
            }
        }

        Ok(matches)
    }

    fn current_revision(&self, topic: &str) -> Result<Option<PageRevision>> {
        match self.read(topic) {
            Ok(page) => Ok(Some(page.revision)),
            Err(Error::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn reject_occupied_destination(&self, topic: &str, path: &Path) -> Result<()> {
        for candidate in [path.to_path_buf(), metadata_path(path)?] {
            match fs::symlink_metadata(&candidate) {
                Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                    return Err(Error::UnsafeSymlink(candidate));
                }
                Ok(_) => return Err(Error::AlreadyExists(topic.to_owned())),
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn available_recovery_path(&self, page_path: &Path, digest: &str) -> Result<(String, PathBuf)> {
        for attempt in 0_u16..=u16::MAX {
            let token = if attempt == 0 {
                digest.to_owned()
            } else {
                format!("{digest}-{attempt}")
            };
            let path = self.recovery_path(page_path, &token)?;
            self.reject_path_components(&path)?;
            let sidecar = metadata_path(&path)?;

            let page_available = matches!(
                fs::symlink_metadata(&path),
                Err(error) if error.kind() == ErrorKind::NotFound
            );
            let sidecar_available = matches!(
                fs::symlink_metadata(&sidecar),
                Err(error) if error.kind() == ErrorKind::NotFound
            );
            if page_available && sidecar_available {
                return Ok((token, path));
            }
        }

        Err(Error::Io(std::io::Error::new(
            ErrorKind::AlreadyExists,
            "could not allocate a unique recovery-copy name",
        )))
    }

    fn recovery_path(&self, page_path: &Path, recovery_token: &str) -> Result<PathBuf> {
        let stem = page_path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(PAGE_SUFFIX))
            .ok_or_else(|| Error::InvalidTopic(page_path.display().to_string()))?;
        let parent = page_path
            .parent()
            .expect("a page path always has a parent directory");
        Ok(parent.join(format!("{stem}{DELETED_MARKER}{recovery_token}.md")))
    }

    fn move_with_sidecar_no_replace(&self, source: &Path, destination: &Path) -> Result<()> {
        self.ensure_parent_directories(destination)?;
        self.reject_path_components(source)?;
        self.reject_path_components(destination)?;
        let source_sidecar = metadata_path(source)?;
        let destination_sidecar = metadata_path(destination)?;

        let source_metadata = fs::symlink_metadata(source)?;
        if is_symlink_or_reparse(&source_metadata) {
            return Err(Error::UnsafeSymlink(source.to_path_buf()));
        }
        if !source_metadata.is_file() {
            return Err(Error::UnsafeFileType(source.to_path_buf()));
        }

        let has_sidecar = match fs::symlink_metadata(&source_sidecar) {
            Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                return Err(Error::UnsafeSymlink(source_sidecar));
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(Error::UnsafeFileType(source_sidecar));
            }
            Ok(_) => true,
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => return Err(error.into()),
        };

        no_replace_rename(source, destination)?;
        if has_sidecar {
            if let Err(error) = no_replace_rename(&source_sidecar, &destination_sidecar) {
                let rollback = no_replace_rename(destination, source);
                return Err(Error::Io(match rollback {
                    Ok(()) => error,
                    Err(rollback_error) => std::io::Error::other(format!(
                        "{error}; page rollback also failed: {rollback_error}"
                    )),
                }));
            }
        }

        Ok(())
    }

    fn ensure_parent_directories(&self, path: &Path) -> Result<()> {
        self.ensure()?;
        let parent = path
            .parent()
            .expect("a page path always has a parent directory");
        let relative = parent
            .strip_prefix(&self.root)
            .expect("page parent stays inside the vault");
        let mut current = self.root.clone();

        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(Error::InvalidTopic(relative.display().to_string()));
            };
            current.push(name);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                    return Err(Error::UnsafeSymlink(current));
                }
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => {
                    return Err(Error::Io(std::io::Error::new(
                        ErrorKind::NotADirectory,
                        format!(
                            "vault path component is not a directory: {}",
                            current.display()
                        ),
                    )));
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    match fs::create_dir(&current) {
                        Ok(()) => {}
                        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                        Err(error) => return Err(error.into()),
                    }
                    let metadata = fs::symlink_metadata(&current)?;
                    if is_symlink_or_reparse(&metadata) {
                        return Err(Error::UnsafeSymlink(current));
                    }
                    if !metadata.is_dir() {
                        return Err(Error::Io(std::io::Error::new(
                            ErrorKind::NotADirectory,
                            format!(
                                "vault path component is not a directory: {}",
                                current.display()
                            ),
                        )));
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }

        Ok(())
    }

    fn reject_path_components(&self, path: &Path) -> Result<()> {
        let relative = path.strip_prefix(&self.root).map_err(|_| {
            Error::Io(std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("path is outside the vault: {}", path.display()),
            ))
        })?;
        let root_metadata = fs::symlink_metadata(&self.root)?;
        if is_symlink_or_reparse(&root_metadata) {
            return Err(Error::UnsafeSymlink(self.root.clone()));
        }

        let mut current = self.root.clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(Error::InvalidTopic(relative.display().to_string()));
            };
            current.push(name);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if is_symlink_or_reparse(&metadata) => {
                    return Err(Error::UnsafeSymlink(current));
                }
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => break,
                Err(error) => return Err(error.into()),
            }
        }

        Ok(())
    }

    fn page_path(&self, topic: &str) -> Result<PathBuf> {
        validate_topic(topic)?;
        let relative = format!("{topic}{PAGE_SUFFIX}");
        Ok(self.root.join(relative))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn no_replace_rename(source: &Path, destination: &Path) -> std::io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE)
        .map_err(std::io::Error::from)
}

#[cfg(windows)]
fn no_replace_rename(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileW;

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();

    // MoveFileW fails when the destination exists; no replace flag is enabled.
    let result = unsafe { MoveFileW(source.as_ptr(), destination.as_ptr()) };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn no_replace_rename(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::hard_link(source, destination)?;
    if let Err(error) = fs::remove_file(source) {
        let _ = fs::remove_file(destination);
        return Err(error.into());
    }
    Ok(())
}

fn conflict(topic: &str, expected_revision: &PageRevision, actual: Option<PageRevision>) -> Error {
    Error::Conflict {
        topic: topic.to_owned(),
        expected: expected_revision.clone(),
        actual,
    }
}

fn metadata_path(page_path: &Path) -> Result<PathBuf> {
    let stem = page_path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| {
            name.strip_suffix(PAGE_SUFFIX)
                .or_else(|| name.strip_suffix(".md"))
        })
        .ok_or_else(|| Error::InvalidTopic(page_path.display().to_string()))?;
    let parent = page_path
        .parent()
        .expect("a page path always has a parent directory");
    Ok(parent.join(format!("{stem}{METADATA_SUFFIX}")))
}

fn stage_atomic_write(path: &Path, content: &str) -> Result<AtomicWriteFile> {
    reject_oversized_page(path, content.len() as u64)?;
    let mut staged = AtomicWriteFile::open(path)?;
    staged.write_all(content.as_bytes())?;
    staged.flush()?;
    staged.sync_all()?;
    Ok(staged)
}

fn read_snapshot(path: &Path) -> Result<(String, PageRevision)> {
    let metadata = fs::symlink_metadata(path)?;
    if is_symlink_or_reparse(&metadata) {
        return Err(Error::UnsafeSymlink(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(Error::UnsafeFileType(path.to_path_buf()));
    }
    reject_oversized_page(path, metadata.len())?;

    let mut file = open_read_nofollow(path)?;
    let metadata = file.metadata()?;
    if is_symlink_or_reparse(&metadata) {
        return Err(Error::UnsafeSymlink(path.to_path_buf()));
    }
    if !metadata.is_file() {
        return Err(Error::UnsafeFileType(path.to_path_buf()));
    }
    reject_oversized_page(path, metadata.len())?;

    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((MAX_PAGE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    reject_oversized_page(path, bytes.len() as u64)?;
    let metadata = file.metadata()?;

    let content = String::from_utf8(bytes).map_err(|error| {
        Error::Io(std::io::Error::new(
            ErrorKind::InvalidData,
            format!("page is not valid UTF-8: {error}"),
        ))
    })?;
    let revision = PageRevision {
        modified_unix_nanos: modified_unix_nanos(&metadata),
        content_sha256: content_sha256(content.as_bytes()),
    };
    Ok((content, revision))
}

fn modified_unix_nanos(metadata: &Metadata) -> Option<String> {
    let duration = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    let nanos =
        u128::from(duration.as_secs()) * 1_000_000_000 + u128::from(duration.subsec_nanos());
    Some(nanos.to_string())
}

fn content_sha256(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn reject_oversized_page(path: &Path, byte_len: u64) -> Result<()> {
    if byte_len > MAX_PAGE_BYTES as u64 {
        return Err(Error::PageTooLarge {
            path: path.to_path_buf(),
            max_bytes: MAX_PAGE_BYTES,
        });
    }
    Ok(())
}

#[cfg(unix)]
fn open_read_nofollow(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
}

#[cfg(windows)]
fn open_read_nofollow(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(0x0020_0000) // FILE_FLAG_OPEN_REPARSE_POINT
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_read_nofollow(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).open(path)
}

#[cfg(windows)]
fn is_symlink_or_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_symlink_or_reparse(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn validate_topic(topic: &str) -> Result<()> {
    if topic.len() > MAX_TOPIC_BYTES {
        return Err(Error::InputTooLarge {
            field: "topic",
            max_bytes: MAX_TOPIC_BYTES,
        });
    }
    if topic.is_empty()
        || topic.starts_with('/')
        || topic.starts_with('\\')
        || topic.contains('\\')
        || topic
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(Error::InvalidTopic(topic.to_owned()));
    }

    let path = Path::new(topic);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::InvalidTopic(topic.to_owned()));
    }

    if topic.ends_with(PAGE_SUFFIX) {
        return Err(Error::InvalidTopic(topic.to_owned()));
    }

    Ok(())
}

fn validate_recovery_token(token: &str) -> Result<()> {
    let mut parts = token.split('-');
    let digest = parts.next().unwrap_or_default();
    let suffix = parts.next();
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || suffix.is_some_and(|value| {
            value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit())
        })
        || parts.next().is_some()
    {
        return Err(Error::InvalidTopic("invalid recovery token".to_owned()));
    }
    Ok(())
}

fn title_from_content(content: &str, fallback: &str) -> String {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_lists_reads_and_searches_pages() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));

        vault
            .create("python/new-project", Some("New Python project"))
            .expect("create page");

        let page = vault.read("python/new-project").expect("read page");
        assert_eq!(page.title, "New Python project");
        assert_eq!(page.revision.content_sha256.len(), 64);
        assert!(page.revision.modified_unix_nanos.is_some());

        let pages = vault.list().expect("list pages");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].topic, "python/new-project");

        let matches = vault.search("python").expect("search pages");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn atomically_replaces_a_page_with_the_expected_revision() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        let original = vault
            .create_with_content("git/rebase", "# Rebase\n\nold\n")
            .expect("create page");

        let saved = vault
            .save("git/rebase", "# Rebase safely\n\nnew\n", &original.revision)
            .expect("save page");

        assert_eq!(saved.content, "# Rebase safely\n\nnew\n");
        assert_ne!(saved.revision, original.revision);
        let entries = fs::read_dir(saved.path.parent().expect("page parent"))
            .expect("read page directory")
            .collect::<std::io::Result<Vec<_>>>()
            .expect("directory entries");
        assert_eq!(entries.len(), 1, "temporary files should be cleaned up");
    }

    #[test]
    fn detects_external_changes_without_overwriting_either_version() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        let original = vault
            .create_with_content("git/rebase", "# Rebase\n\noriginal\n")
            .expect("create page");
        fs::write(&original.path, "# Rebase\n\nexternal\n").expect("external write");

        let result = vault.save("git/rebase", "# Rebase\n\nmy draft\n", &original.revision);
        assert!(matches!(
            result,
            Err(Error::Conflict {
                actual: Some(_),
                ..
            })
        ));
        assert_eq!(
            fs::read_to_string(&original.path).expect("disk page"),
            "# Rebase\n\nexternal\n"
        );

        let copy = vault
            .preserve_conflict_copy("git/rebase", "# Rebase\n\nmy draft\n")
            .expect("preserve draft");
        assert_eq!(
            fs::read_to_string(copy).expect("conflict copy"),
            "# Rebase\n\nmy draft\n"
        );
    }

    #[test]
    fn detects_external_deletion() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        let original = vault
            .create_with_content("git/rebase", "# Rebase\n")
            .expect("create page");
        fs::remove_file(&original.path).expect("external delete");

        assert!(matches!(
            vault.save("git/rebase", "# My draft\n", &original.revision),
            Err(Error::Conflict { actual: None, .. })
        ));
        assert!(!original.path.exists());
    }

    #[test]
    fn renames_a_page_and_its_metadata_without_overwriting() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        let original = vault
            .create_with_content("git/rebase", "# Rebase\n")
            .expect("create page");
        let original_sidecar = vault.root().join("git/rebase.page.meta.yaml");
        fs::write(&original_sidecar, "version: 1\nid: rebase\n").expect("metadata sidecar");

        let renamed = vault
            .rename("git/rebase", "git/safe-rebase", &original.revision)
            .expect("rename page");

        assert_eq!(renamed.topic, "git/safe-rebase");
        assert!(!original.path.exists());
        assert!(!original_sidecar.exists());
        assert_eq!(
            fs::read_to_string(vault.root().join("git/safe-rebase.page.meta.yaml"))
                .expect("renamed metadata"),
            "version: 1\nid: rebase\n"
        );

        let occupied = vault
            .create_with_content("git/existing", "# Existing\n")
            .expect("occupied page");
        assert!(matches!(
            vault.rename("git/safe-rebase", "git/existing", &renamed.revision),
            Err(Error::AlreadyExists(topic)) if topic == "git/existing"
        ));
        assert_eq!(vault.read("git/existing").expect("existing page"), occupied);
        assert!(vault.read("git/safe-rebase").is_ok());
    }

    #[test]
    fn rename_requires_the_last_read_revision() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        let original = vault
            .create_with_content("git/rebase", "# Rebase\n")
            .expect("create page");
        fs::write(&original.path, "# Changed elsewhere\n").expect("external edit");

        assert!(matches!(
            vault.rename("git/rebase", "git/safe-rebase", &original.revision),
            Err(Error::Conflict {
                actual: Some(_),
                ..
            })
        ));
        assert!(original.path.exists());
        assert!(!vault.root().join("git/safe-rebase.page.md").exists());
    }

    #[test]
    fn delete_is_recoverable_and_carries_metadata() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        let original = vault
            .create_with_content("git/rebase", "# Rebase\n")
            .expect("create page");
        fs::write(
            vault.root().join("git/rebase.page.meta.yaml"),
            "version: 1\n",
        )
        .expect("metadata sidecar");

        let deleted = vault
            .delete("git/rebase", &original.revision)
            .expect("recoverable delete");

        assert!(!original.path.exists());
        assert!(matches!(vault.read("git/rebase"), Err(Error::NotFound(_))));
        assert!(deleted.recovery_path.exists());
        assert!(vault.list().expect("list pages").is_empty());
        assert!(
            deleted
                .recovery_path
                .with_file_name(format!(
                    "{}.page.meta.yaml",
                    deleted
                        .recovery_path
                        .file_stem()
                        .expect("recovery stem")
                        .to_string_lossy()
                ))
                .exists()
        );

        let restored = vault
            .restore_deleted("git/rebase", &deleted.recovery_token)
            .expect("restore deleted page");
        assert_eq!(restored.content, original.content);
        assert!(!deleted.recovery_path.exists());
        assert_eq!(
            fs::read_to_string(vault.root().join("git/rebase.page.meta.yaml"))
                .expect("restored metadata"),
            "version: 1\n"
        );
    }

    #[test]
    fn restore_deleted_does_not_replace_a_recreated_page() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        let original = vault
            .create_with_content("git/rebase", "# Original\n")
            .expect("create page");
        let deleted = vault
            .delete("git/rebase", &original.revision)
            .expect("recoverable delete");
        let recreated = vault
            .create_with_content("git/rebase", "# Recreated\n")
            .expect("recreate page");

        assert!(matches!(
            vault.restore_deleted("git/rebase", &deleted.recovery_token),
            Err(Error::AlreadyExists(topic)) if topic == "git/rebase"
        ));
        assert_eq!(vault.read("git/rebase").expect("recreated page"), recreated);
        assert!(deleted.recovery_path.exists());
    }

    #[test]
    fn sidecar_move_failure_rolls_the_page_back() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        let original = vault
            .create_with_content("git/rebase", "# Original\n")
            .expect("create page");
        let source_sidecar = vault.root().join("git/rebase.page.meta.yaml");
        fs::write(&source_sidecar, "version: 1\n").expect("source sidecar");
        let destination = vault.root().join("git/renamed.page.md");
        let destination_sidecar = vault.root().join("git/renamed.page.meta.yaml");
        fs::write(&destination_sidecar, "occupied\n").expect("occupied sidecar");

        assert!(matches!(
            vault.move_with_sidecar_no_replace(&original.path, &destination),
            Err(Error::Io(_))
        ));
        assert_eq!(
            fs::read_to_string(&original.path).expect("rolled back page"),
            "# Original\n"
        );
        assert_eq!(
            fs::read_to_string(source_sidecar).expect("source metadata"),
            "version: 1\n"
        );
        assert!(!destination.exists());
        assert_eq!(
            fs::read_to_string(destination_sidecar).expect("occupied metadata"),
            "occupied\n"
        );
    }

    #[test]
    fn conflict_copy_is_stable_for_the_same_content() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        vault
            .create_with_content("git/rebase", "# Rebase\n")
            .expect("create page");

        let first = vault
            .preserve_conflict_copy("git/rebase", "draft")
            .expect("first conflict copy");
        let second = vault
            .preserve_conflict_copy("git/rebase", "draft")
            .expect("second conflict copy");

        assert_eq!(first, second);
        assert!(!first.to_string_lossy().ends_with(PAGE_SUFFIX));
        assert_eq!(vault.list().expect("list pages").len(), 1);
    }

    #[test]
    fn rejects_portable_path_attacks() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));

        for topic in [
            "../escape",
            "nested/../../escape",
            "/absolute",
            r"\absolute",
            r"nested\..\escape",
            "double//separator",
            "./relative",
        ] {
            assert!(
                matches!(vault.create(topic, None), Err(Error::InvalidTopic(_))),
                "topic should be rejected: {topic}"
            );
        }
    }

    #[test]
    fn rejects_leaf_symlinks_without_modifying_the_target() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault_root = directory.path().join("vault");
        let outside = directory.path().join("outside.md");
        fs::create_dir(&vault_root).expect("vault directory");
        fs::write(&outside, "outside").expect("outside page");
        let link = vault_root.join("linked.page.md");
        if let Err(error) = symlink_file(&outside, &link) {
            if cfg!(windows) && error.kind() == ErrorKind::PermissionDenied {
                return;
            }
            panic!("create file symlink: {error}");
        }
        let vault = Vault::new(&vault_root);

        assert!(matches!(vault.read("linked"), Err(Error::UnsafeSymlink(_))));
        assert!(matches!(
            vault.create_with_content("linked", "replacement"),
            Err(Error::UnsafeSymlink(_))
        ));
        assert_eq!(
            fs::read_to_string(outside).expect("outside page"),
            "outside"
        );
    }

    #[test]
    fn rejects_parent_directory_symlinks() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault_root = directory.path().join("vault");
        let outside = directory.path().join("outside");
        fs::create_dir(&vault_root).expect("vault directory");
        fs::create_dir(&outside).expect("outside directory");
        let link = vault_root.join("nested");
        if let Err(error) = symlink_dir(&outside, &link) {
            if cfg!(windows) && error.kind() == ErrorKind::PermissionDenied {
                return;
            }
            panic!("create directory symlink: {error}");
        }
        let vault = Vault::new(&vault_root);

        assert!(matches!(
            vault.create_with_content("nested/escape", "replacement"),
            Err(Error::UnsafeSymlink(_))
        ));
        assert!(!outside.join("escape.page.md").exists());
    }

    #[test]
    fn allows_a_platform_symlink_above_the_vault_boundary() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let physical_parent = directory.path().join("physical-parent");
        fs::create_dir(&physical_parent).expect("physical parent");
        let linked_parent = directory.path().join("linked-parent");
        if let Err(error) = symlink_dir(&physical_parent, &linked_parent) {
            if cfg!(windows) && error.kind() == ErrorKind::PermissionDenied {
                return;
            }
            panic!("create directory symlink: {error}");
        }
        let vault = Vault::new(linked_parent.join("vault"));

        let page = vault
            .create_with_content("safe", "# Safe\n")
            .expect("create below the trusted vault boundary");
        assert_eq!(page.content, "# Safe\n");
        assert!(physical_parent.join("vault/safe.page.md").is_file());
    }

    #[test]
    fn rejects_a_symlink_as_the_vault_root() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let physical_vault = directory.path().join("physical-vault");
        fs::create_dir(&physical_vault).expect("physical vault");
        let linked_vault = directory.path().join("linked-vault");
        if let Err(error) = symlink_dir(&physical_vault, &linked_vault) {
            if cfg!(windows) && error.kind() == ErrorKind::PermissionDenied {
                return;
            }
            panic!("create directory symlink: {error}");
        }
        let vault = Vault::new(&linked_vault);

        assert!(matches!(vault.ensure(), Err(Error::UnsafeSymlink(_))));
    }

    #[test]
    fn rejects_non_regular_page_paths() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault_root = directory.path().join("vault");
        fs::create_dir(&vault_root).expect("vault directory");
        fs::create_dir(vault_root.join("directory.page.md")).expect("page-shaped directory");
        let vault = Vault::new(&vault_root);

        assert!(matches!(
            vault.read("directory"),
            Err(Error::UnsafeFileType(_))
        ));
    }

    #[test]
    fn rejects_oversized_page_content_before_writing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        let oversized = "x".repeat(MAX_PAGE_BYTES + 1);

        assert!(matches!(
            vault.create_with_content("oversized", &oversized),
            Err(Error::PageTooLarge { .. })
        ));
        assert!(!vault.root().join("oversized.page.md").exists());
    }

    #[test]
    fn rejects_an_oversized_page_added_outside_myhelp() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));
        vault.ensure().expect("vault directory");
        fs::write(
            vault.root().join("oversized.page.md"),
            vec![b'x'; MAX_PAGE_BYTES + 1],
        )
        .expect("external oversized page");

        assert!(matches!(
            vault.read("oversized"),
            Err(Error::PageTooLarge { .. })
        ));
    }

    #[test]
    fn rejects_oversized_topics_and_search_queries() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path().join("vault"));

        assert!(matches!(
            vault.create(&"t".repeat(MAX_TOPIC_BYTES + 1), None),
            Err(Error::InputTooLarge { field: "topic", .. })
        ));
        assert!(matches!(
            vault.search(&"q".repeat(MAX_SEARCH_QUERY_BYTES + 1)),
            Err(Error::InputTooLarge {
                field: "search query",
                ..
            })
        ));
    }

    #[cfg(unix)]
    fn symlink_file(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(original, link)
    }

    #[cfg(unix)]
    fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(original, link)
    }

    #[cfg(windows)]
    fn symlink_file(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(original, link)
    }

    #[cfg(windows)]
    fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(original, link)
    }
}
