#[tauri::command]
fn list_pages() -> Result<Vec<myhelp_core::PageSummary>, String> {
    vault()?.list().map_err(|error| error.to_string())
}

#[tauri::command]
fn read_page(topic: String) -> Result<myhelp_core::Page, String> {
    vault()?.read(&topic).map_err(|error| error.to_string())
}

#[tauri::command]
fn save_page(topic: String, content: String) -> Result<myhelp_core::Page, String> {
    vault()?
        .write(&topic, &content)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_page(topic: String, title: Option<String>) -> Result<myhelp_core::Page, String> {
    vault()?
        .create(&topic, title.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn search_pages(query: String) -> Result<Vec<myhelp_core::PageSummary>, String> {
    vault()?.search(&query).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_vault_path() -> Result<String, String> {
    Ok(vault()?.root().display().to_string())
}

fn vault() -> Result<myhelp_core::Vault, String> {
    myhelp_core::Vault::discover().map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_pages,
            read_page,
            save_page,
            create_page,
            search_pages,
            get_vault_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
