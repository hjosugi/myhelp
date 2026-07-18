use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

const PAGE_SUFFIX: &str = ".page.md";

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
pub struct Page {
    pub topic: String,
    pub title: String,
    pub content: String,
    pub path: PathBuf,
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
        fs::create_dir_all(&self.root)?;
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
            let content = fs::read_to_string(entry.path())?;
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
        if !path.is_file() {
            return Err(Error::NotFound(topic.to_owned()));
        }

        let content = fs::read_to_string(&path)?;
        Ok(Page {
            topic: topic.to_owned(),
            title: title_from_content(&content, topic),
            content,
            path,
        })
    }

    pub fn create(&self, topic: &str, title: Option<&str>) -> Result<Page> {
        let path = self.page_path(topic)?;
        if path.exists() {
            return Err(Error::AlreadyExists(topic.to_owned()));
        }

        let title = title.unwrap_or(topic);
        let content = format!(
            "# {title}\n\n> Personal help for {title}.\n\n- Add an example:\n\n`command --option`\n"
        );
        self.write(topic, &content)
    }

    pub fn write(&self, topic: &str, content: &str) -> Result<Page> {
        let path = self.page_path(topic)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, content)?;
        Ok(Page {
            topic: topic.to_owned(),
            title: title_from_content(content, topic),
            content: content.to_owned(),
            path,
        })
    }

    pub fn search(&self, query: &str) -> Result<Vec<PageSummary>> {
        let query = query.to_lowercase();
        let mut matches = Vec::new();

        for summary in self.list()? {
            let content = fs::read_to_string(&summary.path)?;
            if summary.topic.to_lowercase().contains(&query)
                || summary.title.to_lowercase().contains(&query)
                || content.to_lowercase().contains(&query)
            {
                matches.push(summary);
            }
        }

        Ok(matches)
    }

    fn page_path(&self, topic: &str) -> Result<PathBuf> {
        validate_topic(topic)?;
        let relative = format!("{topic}{PAGE_SUFFIX}");
        Ok(self.root.join(relative))
    }
}

fn validate_topic(topic: &str) -> Result<()> {
    if topic.is_empty() || topic.starts_with('/') || topic.starts_with('\\') {
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
        let vault = Vault::new(directory.path());

        vault
            .create("python/new-project", Some("New Python project"))
            .expect("create page");

        let page = vault.read("python/new-project").expect("read page");
        assert_eq!(page.title, "New Python project");

        let pages = vault.list().expect("list pages");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].topic, "python/new-project");

        let matches = vault.search("python").expect("search pages");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn rejects_parent_directory_topics() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let vault = Vault::new(directory.path());

        assert!(matches!(
            vault.create("../escape", None),
            Err(Error::InvalidTopic(_))
        ));
    }
}
