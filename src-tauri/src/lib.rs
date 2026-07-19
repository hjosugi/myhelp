use myhelp_core::{DeletedPage, Error as CoreError, Page, PageRevision, PageSummary, Vault};
use notify::{Config, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::path::Path;
use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::Duration;
use tauri::plugin::{Builder as PluginBuilder, TauriPlugin};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, Url, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

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
            CoreError::PageTooLarge { .. } => ("pageTooLarge", None),
            CoreError::InputTooLarge { .. } => ("inputTooLarge", None),
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

    fn storage(message: impl Into<String>) -> Self {
        Self {
            kind: "storage",
            message: message.into(),
            draft_path: None,
            actual_revision: None,
        }
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

struct DesktopState {
    vault: Mutex<Vault>,
    watcher: Mutex<VaultWatcher>,
}

impl DesktopState {
    fn current_vault(&self) -> Result<Vault, CommandError> {
        self.vault
            .lock()
            .map(|vault| vault.clone())
            .map_err(|_| CommandError::storage("vault state lock was poisoned"))
    }
}

#[tauri::command]
fn list_pages(state: State<'_, DesktopState>) -> Result<Vec<PageSummary>, CommandError> {
    state
        .current_vault()?
        .list()
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn read_page(state: State<'_, DesktopState>, topic: String) -> Result<Page, CommandError> {
    state
        .current_vault()?
        .read(&topic)
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn save_page(
    state: State<'_, DesktopState>,
    topic: String,
    content: String,
    expected_revision: PageRevision,
) -> Result<Page, CommandError> {
    let vault = state.current_vault()?;
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
fn preserve_draft(
    state: State<'_, DesktopState>,
    topic: String,
    content: String,
) -> Result<String, CommandError> {
    state
        .current_vault()?
        .preserve_conflict_copy(&topic, &content)
        .map(|path| path.display().to_string())
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn restore_page(
    state: State<'_, DesktopState>,
    topic: String,
    content: String,
) -> Result<Page, CommandError> {
    state
        .current_vault()?
        .create_with_content(&topic, &content)
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn create_page(
    state: State<'_, DesktopState>,
    topic: String,
    title: Option<String>,
) -> Result<Page, CommandError> {
    state
        .current_vault()?
        .create(&topic, title.as_deref())
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn search_pages(
    state: State<'_, DesktopState>,
    query: String,
) -> Result<Vec<PageSummary>, CommandError> {
    state
        .current_vault()?
        .search(&query)
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn get_vault_path(state: State<'_, DesktopState>) -> Result<String, CommandError> {
    Ok(state.current_vault()?.root().display().to_string())
}

#[tauri::command]
fn rename_page(
    state: State<'_, DesktopState>,
    topic: String,
    new_topic: String,
    expected_revision: PageRevision,
) -> Result<Page, CommandError> {
    state
        .current_vault()?
        .rename(&topic, &new_topic, &expected_revision)
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn delete_page(
    state: State<'_, DesktopState>,
    topic: String,
    expected_revision: PageRevision,
) -> Result<DeletedPage, CommandError> {
    state
        .current_vault()?
        .delete(&topic, &expected_revision)
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
fn restore_deleted_page(
    state: State<'_, DesktopState>,
    topic: String,
    recovery_token: String,
) -> Result<Page, CommandError> {
    state
        .current_vault()?
        .restore_deleted(&topic, &recovery_token)
        .map_err(|error| CommandError::from_core(&error))
}

#[tauri::command]
async fn choose_vault(
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<Option<String>, CommandError> {
    let current = state.current_vault()?.root().to_path_buf();
    let Some(selected) = app
        .dialog()
        .file()
        .set_title("Choose a MyHelp vault")
        .set_directory(current)
        .blocking_pick_folder()
    else {
        return Ok(None);
    };
    let path = selected
        .into_path()
        .map_err(|error| CommandError::storage(format!("invalid vault path: {error}")))?;
    let vault = Vault::new(&path);
    vault
        .ensure()
        .map_err(|error| CommandError::from_core(&error))?;
    let watcher = start_vault_watcher(&app, &vault)
        .map_err(|error| CommandError::storage(format!("could not watch vault: {error}")))?;

    *state
        .vault
        .lock()
        .map_err(|_| CommandError::storage("vault state lock was poisoned"))? = vault;
    *state
        .watcher
        .lock()
        .map_err(|_| CommandError::storage("watcher state lock was poisoned"))? = watcher;

    Ok(Some(path.display().to_string()))
}

#[tauri::command]
fn close_window(window: WebviewWindow) -> Result<(), CommandError> {
    window
        .destroy()
        .map_err(|error| CommandError::storage(format!("could not close window: {error}")))
}

fn navigation_guard<R: Runtime>() -> TauriPlugin<R> {
    PluginBuilder::<R, ()>::new("myhelp-navigation-guard")
        .on_navigation(|_, url| navigation_allowed(url, cfg!(debug_assertions)))
        .build()
}

fn navigation_allowed(url: &Url, allow_dev_server: bool) -> bool {
    let bundled_asset = url.scheme() == "tauri"
        || (matches!(url.scheme(), "http" | "https")
            && url.host_str() == Some("tauri.localhost")
            && url.port().is_none());
    let dev_asset = allow_dev_server
        && url.scheme() == "http"
        && url.host_str() == Some("localhost")
        && url.port() == Some(1420);

    bundled_asset || dev_asset
}

fn start_vault_watcher(
    app: &AppHandle,
    vault: &Vault,
) -> Result<VaultWatcher, Box<dyn std::error::Error>> {
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
        .plugin(navigation_guard())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let vault = Vault::discover()?;
            let watcher = start_vault_watcher(app.handle(), &vault)?;
            app.manage(DesktopState {
                vault: Mutex::new(vault),
                watcher: Mutex::new(watcher),
            });
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
            get_vault_path,
            rename_page,
            delete_page,
            restore_deleted_page,
            choose_vault,
            close_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_navigation_allows_only_bundled_assets() {
        for url in [
            "tauri://localhost/index.html",
            "http://tauri.localhost/index.html",
            "https://tauri.localhost/index.html",
        ] {
            assert!(navigation_allowed(&url.parse().expect("valid URL"), false));
        }

        for url in [
            "https://example.com/",
            "http://localhost:1420/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,unsafe",
            "about:blank",
            "http://tauri.localhost:8080/",
        ] {
            assert!(
                !navigation_allowed(&url.parse().expect("valid URL"), false),
                "production navigation should reject {url}"
            );
        }
    }

    #[test]
    fn development_navigation_allows_only_the_configured_vite_origin() {
        assert!(navigation_allowed(
            &"http://localhost:1420/".parse().expect("valid URL"),
            true
        ));

        for url in [
            "http://127.0.0.1:1420/",
            "http://localhost:1421/",
            "https://localhost:1420/",
            "https://example.com/",
        ] {
            assert!(
                !navigation_allowed(&url.parse().expect("valid URL"), true),
                "development navigation should reject {url}"
            );
        }
    }

    #[test]
    fn security_configuration_keeps_development_exceptions_out_of_production() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");
        let security = &config["app"]["security"];
        let production = security["csp"].to_string();
        let development = security["devCsp"].to_string();

        assert_eq!(security["capabilities"], serde_json::json!(["main-editor"]));
        assert_eq!(security["freezePrototype"], true);
        for forbidden in [
            "localhost:1420",
            "ws:",
            "unsafe-eval",
            "unsafe-inline",
            "https://",
        ] {
            assert!(
                !production.contains(forbidden),
                "production CSP must not contain {forbidden}"
            );
        }
        assert!(development.contains("ws://localhost:1420"));
        assert!(development.contains("unsafe-eval"));
        assert!(development.contains("unsafe-inline"));
    }

    #[test]
    fn main_capability_grants_only_events_and_typed_myhelp_commands() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json"))
                .expect("valid capability");
        let permissions = capability["permissions"]
            .as_array()
            .expect("permission array")
            .iter()
            .map(|permission| permission.as_str().expect("permission string"))
            .collect::<Vec<_>>();

        assert_eq!(capability["identifier"], "main-editor");
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert_eq!(
            permissions,
            [
                "core:event:allow-listen",
                "core:event:allow-unlisten",
                "allow-list-pages",
                "allow-read-page",
                "allow-save-page",
                "allow-preserve-draft",
                "allow-restore-page",
                "allow-create-page",
                "allow-search-pages",
                "allow-get-vault-path",
                "allow-rename-page",
                "allow-delete-page",
                "allow-restore-deleted-page",
                "allow-choose-vault",
                "allow-close-window",
            ]
        );
    }
}
