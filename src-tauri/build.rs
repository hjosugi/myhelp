fn main() {
    const COMMANDS: &[&str] = &[
        "list_pages",
        "read_page",
        "save_page",
        "preserve_draft",
        "restore_page",
        "create_page",
        "search_pages",
        "get_vault_path",
        "rename_page",
        "delete_page",
        "restore_deleted_page",
        "choose_vault",
        "close_window",
    ];

    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    tauri_build::try_build(attributes).expect("failed to build Tauri application metadata");
}
