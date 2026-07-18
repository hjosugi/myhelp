use myhelp_core::{Error as CoreError, Page, PageRevision, PageSummary, Vault};
use notify::{Config, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::path::Path;
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandError {
    kind: &'static str,
    message: String,
    draft_path: Option<String>,
    actual_revision: Option<PageRevision>,
}

impl CommandError {
    fn from_core(error: &CoreError) -> Self {
        let (kind, actual_revision) = match error {
            CoreError::Conflict { actual, .. } => ("conflict", actual.clone()),
            CoreError::NotFound(_) => ("notFound", None),
            CoreError::AlreadyExists(_) => ("alreadyExists", None),
            CoreError::InvalidTopic(_) => ("invalidTopic", None),
            CoreError::UnsafeSymlink(_) | CoreError::UnsafeFileType(_) => ("unsafePath", None),
            CoreError::MissingDataDirectory | CoreError::Io(_) | CoreError::WalkDir(_) => {
                ("storage", None)
            }
        };
        Self {
            kind,
            message: error.to_string(),
            draft_path: None,
            actual_revision,
        }
    }

    fn with_draft_path(mut self, path: &Path) -> Self {
        self.draft_path = Some(path.display().to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultChanged {
    paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultWatchError {
    message: String,
}

struct VaultWatcher {
    _watcher: Mutex<Box<dyn Watcher + Send>>,
    _forwarder: thread::JoinHandle<()>,
}

#[tauri::command]
fn list_pages() -> Result<Vec<PageSummary>, CommandError> {
    vault()
        .and_then(|vault| vault.list())
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn read_page(topic: String) -> Result<Page, CommandError> {
    vault()
        .and_then(|vault| vault.read(&topic))
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn save_page(
    topic: String,
    content: String,
    expected_revision: PageRevision,
) -> Result<Page, CommandError> {
    let vault = vault().map_err(|error| CommandError::from_core(&error))?;
    match vault.save(&topic, &content, &expected_revision) {
        Ok(page) => Ok(page),
        Err(error @ CoreError::Conflict { .. }) => {
            let command_error = CommandError::from_core(&error);
            match vault.preserve_conflict_copy(&topic, &content) {
                Ok(path) => Err(command_error.with_draft_path(&path)),
                Err(preserve_error) => Err(CommandError {
                    message: format!(
                        "{error}; the in-memory draft could not be copied to disk: {preserve_error}"
                    ),
                    ..command_error
                }),
            }
        }
        Err(error) => Err(CommandError::from_core(&error)),
    }
}

#[tauri::command]
fn preserve_draft(topic: String, content: String) -> Result<String, CommandError> {
    vault()
        .and_then(|vault| vault.preserve_conflict_copy(&topic, &content))
        .map(|path| path.display().to_string())
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn restore_page(topic: String, content: String) -> Result<Page, CommandError> {
    vault()
        .and_then(|vault| vault.create_with_content(&topic, &content))
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn create_page(topic: String, title: Option<String>) -> Result<Page, CommandError> {
    vault()
        .and_then(|vault| vault.create(&topic, title.as_deref()))
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn search_pages(query: String) -> Result<Vec<PageSummary>, CommandError> {
    vault()
        .and_then(|vault| vault.search(&query))
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn get_vault_path() -> Result<String, CommandError> {
    vault()
        .map(|vault| vault.root().display().to_string())
        .map_err(|error| CommandError::from_core(&error))
}

fn vault() -> myhelp_core::Result<Vault> {
    Vault::discover()
}

fn start_vault_watcher(app: &AppHandle) -> Result<VaultWatcher, Box<dyn std::error::Error>> {
    let vault = Vault::discover()?;
    vault.ensure()?;
    let root = vault.root().to_path_buf();
    let (sender, receiver) = mpsc::channel();

    let native = (|| -> notify::Result<RecommendedWatcher> {
        let mut watcher = RecommendedWatcher::new(
            sender.clone(),
            Config::default().with_follow_symlinks(false),
        )?;
        watcher.watch(&root, RecursiveMode::Recursive)?;
        Ok(watcher)
    })();

    let watcher: Box<dyn Watcher + Send> = match native {
        Ok(watcher) => Box::new(watcher),
        Err(native_error) => {
            eprintln!(
                "native vault watcher unavailable ({native_error}); using content-aware polling"
            );
            let config = Config::default()
                .with_follow_symlinks(false)
                .with_poll_interval(Duration::from_secs(2))
                .with_compare_contents(true);
            let mut watcher = PollWatcher::new(sender, config)?;
            watcher.watch(&root, RecursiveMode::Recursive)?;
            Box::new(watcher)
        }
    };

    let app = app.clone();
    let forwarder = thread::spawn(move || {
        for result in receiver {
            match result {
                Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                    let paths = event
                        .paths
                        .iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect();
                    let _ = app.emit("vault-changed", VaultChanged { paths });
                }
                Ok(_) => {}
                Err(error) => {
                    let _ = app.emit(
                        "vault-watch-error",
                        VaultWatchError {
                            message: error.to_string(),
                        },
                    );
                }
            }
        }
    });

    Ok(VaultWatcher {
        _watcher: Mutex::new(watcher),
        _forwarder: forwarder,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let watcher = start_vault_watcher(app.handle())?;
            app.manage(watcher);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_pages,
            read_page,
            save_page,
            preserve_draft,
            restore_page,
            create_page,
            search_pages,
            get_vault_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
