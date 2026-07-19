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
