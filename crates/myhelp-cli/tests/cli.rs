use serde_json::Value;
use std::fs;
use std::io::Read;
use std::process::Stdio;
use std::process::{Command, Output};

fn myhelp(arguments: &[&str], pages: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_myhelp"))
        .arg("--pages-dir")
        .arg(pages)
        .args(arguments)
        .output()
        .expect("run myhelp")
}

fn fixture_vault() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary directory");
    fs::write(
        directory.path().join("git.page.md"),
        "# Git\n\n> Distributed version control.\n\n- Show status:\n\n`git status`\n",
    )
    .expect("git page");
    fs::write(
        directory.path().join("rust.page.md"),
        "# Rust\n\n> A systems programming language.",
    )
    .expect("rust page");
    directory
}

#[test]
fn show_is_exact_raw_markdown_when_stdout_is_piped() {
    let vault = fixture_vault();
    let output = myhelp(&["show", "git"], vault.path());
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 stdout"),
        "# Git\n\n> Distributed version control.\n\n- Show status:\n\n`git status`\n"
    );

    let without_trailing_newline = myhelp(&["show", "rust"], vault.path());
    assert!(without_trailing_newline.status.success());
    assert_eq!(
        String::from_utf8(without_trailing_newline.stdout).expect("UTF-8 stdout"),
        "# Rust\n\n> A systems programming language."
    );
}

#[test]
fn list_and_search_offer_deterministic_json() {
    let vault = fixture_vault();

    let text = myhelp(&["list"], vault.path());
    assert!(text.status.success());
    assert_eq!(
        String::from_utf8(text.stdout).expect("text list"),
        "git\tGit\nrust\tRust\n"
    );

    let listed = myhelp(&["list", "--json"], vault.path());
    assert!(listed.status.success());
    let list: Value = serde_json::from_slice(&listed.stdout).expect("list JSON");
    assert_eq!(list.as_array().expect("array").len(), 2);
    assert_eq!(list[0]["topic"], "git");

    let searched = myhelp(&["search", "systems", "--json"], vault.path());
    assert!(searched.status.success());
    let search: Value = serde_json::from_slice(&searched.stdout).expect("search JSON");
    assert_eq!(search.as_array().expect("array").len(), 1);
    assert_eq!(search[0]["topic"], "rust");

    let shown = myhelp(&["show", "git", "--json"], vault.path());
    assert!(shown.status.success());
    let page: Value = serde_json::from_slice(&shown.stdout).expect("page JSON");
    assert_eq!(page["topic"], "git");
    assert_eq!(
        page["content"],
        "# Git\n\n> Distributed version control.\n\n- Show status:\n\n`git status`\n"
    );
}

#[test]
fn generated_completions_cover_every_documented_shell() {
    let missing_vault = tempfile::tempdir().expect("temporary directory");
    let missing_path = missing_vault.path().join("does-not-exist");
    for shell in ["bash", "fish", "zsh", "powershell", "elvish"] {
        let output = myhelp(&["completions", shell], &missing_path);
        assert!(output.status.success(), "{shell}: {:?}", output.stderr);
        assert!(!output.stdout.is_empty(), "{shell}");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("tldr"),
            "{shell} completion must include the tldr adapter"
        );
    }
}

#[test]
fn generated_completions_tolerate_a_downstream_reader_exiting_early() {
    let missing_vault = tempfile::tempdir().expect("temporary directory");
    let mut child = Command::new(env!("CARGO_BIN_EXE_myhelp"))
        .arg("--pages-dir")
        .arg(missing_vault.path().join("does-not-exist"))
        .args(["completions", "fish"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("run myhelp");
    let mut stdout = child.stdout.take().expect("captured stdout");
    let mut prefix = [0_u8; 16];
    stdout.read_exact(&mut prefix).expect("completion prefix");
    drop(stdout);

    assert!(
        child.wait().expect("myhelp status").success(),
        "a normal early consumer exit is successful"
    );
}

#[test]
fn stable_exit_codes_cover_missing_invalid_and_noninteractive_pick() {
    let vault = fixture_vault();

    assert_eq!(
        myhelp(&["show", "missing"], vault.path()).status.code(),
        Some(3)
    );
    assert_eq!(
        myhelp(&["show", "../escape"], vault.path()).status.code(),
        Some(5)
    );
    assert_eq!(
        myhelp(&["pick"], vault.path()).status.code(),
        Some(5),
        "pick must not try to control a noninteractive terminal"
    );
}

#[test]
fn tldr_validation_is_line_oriented_and_does_not_require_a_vault() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let valid = directory.path().join("git.page.md");
    fs::write(
        &valid,
        "# Git\n\n> Work with repositories.\n\n- Show status:\n\n`git status`\n",
    )
    .expect("valid tldr page");
    let missing_vault = directory.path().join("missing-vault");

    let valid_output = myhelp(
        &[
            "tldr",
            "validate",
            valid.to_str().expect("UTF-8 path"),
            "--json",
        ],
        &missing_vault,
    );
    assert!(valid_output.status.success());
    let report: Value = serde_json::from_slice(&valid_output.stdout).expect("validation JSON");
    assert_eq!(report["valid"], true);

    let invalid = directory.path().join("broken.page.md");
    fs::write(&invalid, "# Broken\n\n- Missing command:\n").expect("invalid tldr page");
    let invalid_output = myhelp(
        &[
            "tldr",
            "validate",
            invalid.to_str().expect("UTF-8 path"),
            "--json",
        ],
        &missing_vault,
    );
    assert_eq!(invalid_output.status.code(), Some(5));
    let report: Value = serde_json::from_slice(&invalid_output.stdout).expect("invalid JSON");
    assert_eq!(report["valid"], false);
    assert!(
        report["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .all(|diagnostic| diagnostic["line"].as_u64().is_some_and(|line| line > 0))
    );
}

#[test]
fn tldr_import_and_export_preserve_content_metadata_and_mapping() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let vault = directory.path().join("vault");
    let source = directory.path().join("git.page.md");
    let content = "# Git\n\n> Work with repositories.\n\n- Show status:\n\n`git status`\n";
    fs::write(&source, content).expect("source page");

    let imported = myhelp(
        &[
            "tldr",
            "import",
            source.to_str().expect("UTF-8 path"),
            "--topic",
            "work/git",
            "--source-url",
            "https://github.com/tldr-pages/tldr",
            "--source-license",
            "CC-BY-4.0",
            "--attribution",
            "tldr-pages contributors",
            "--json",
        ],
        &vault,
    );
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    let import_report: Value = serde_json::from_slice(&imported.stdout).expect("import JSON");
    assert_eq!(import_report["topic"], "work/git");
    assert_eq!(
        fs::read_to_string(vault.join("work/git.page.md")).expect("imported content"),
        content
    );
    assert!(vault.join("work/git.page.meta.yaml").is_file());

    let destination = directory.path().join("export");
    let exported = myhelp(
        &[
            "tldr",
            "export",
            destination.to_str().expect("UTF-8 path"),
            "--json",
        ],
        &vault,
    );
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let export_report: Value = serde_json::from_slice(&exported.stdout).expect("export JSON");
    assert_eq!(export_report["mappings"][0]["topic"], "work/git");
    assert_eq!(
        fs::read_to_string(destination.join("work-git.page.md")).expect("exported content"),
        content
    );
    assert!(destination.join("work-git.page.meta.yaml").is_file());
}
