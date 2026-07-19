use anyhow::{Context, Result};
use myhelp_core::{Error as CoreError, Vault};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use tempfile::{Builder, NamedTempFile};

use crate::CliFailure;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditorSyntax {
    Unix,
    Windows,
}

pub(crate) fn edit_page(vault: &Vault, topic: &str) -> Result<()> {
    let page = vault.read(topic)?;
    let mut temporary = Builder::new()
        .prefix("myhelp-")
        .suffix(".page.md")
        .tempfile()
        .context("could not create an editor draft")?;
    temporary
        .write_all(page.content.as_bytes())
        .context("could not initialize the editor draft")?;
    temporary
        .flush()
        .context("could not flush the editor draft")?;
    temporary
        .as_file()
        .sync_all()
        .context("could not sync the editor draft")?;

    let status = open_editor(temporary.path())?;
    let edited = fs::read_to_string(temporary.path()).context("could not read the editor draft")?;

    if !status.success() {
        if edited == page.content {
            return Err(CliFailure::EditorExited(status.to_string()).into());
        }
        let preserved = preserve_draft(vault, topic, &edited, temporary)?;
        return Err(CliFailure::EditorExitedWithDraft {
            status: status.to_string(),
            path: preserved,
        }
        .into());
    }

    if edited == page.content {
        write_status(&format!("unchanged {}", page.path.display()))?;
        return Ok(());
    }

    match vault.save(topic, &edited, &page.revision) {
        Ok(saved) => {
            write_status(&format!("saved {}", saved.path.display()))?;
            Ok(())
        }
        Err(error) if matches!(error, CoreError::Conflict { .. }) => {
            let change = match &error {
                CoreError::Conflict {
                    actual: Some(_), ..
                } => "changed",
                CoreError::Conflict { actual: None, .. } => "was deleted",
                _ => unreachable!("guard only accepts conflicts"),
            };
            let preserved = preserve_draft(vault, topic, &edited, temporary)?;
            Err(error).with_context(|| {
                format!(
                    "the disk page {change}; it was left untouched and your draft was preserved at {}",
                    preserved.display()
                )
            })
        }
        Err(error) => {
            let preserved = preserve_draft(vault, topic, &edited, temporary)?;
            Err(error).with_context(|| {
                format!(
                    "the page was left untouched and your draft was preserved at {}",
                    preserved.display()
                )
            })
        }
    }
}

fn write_status(message: &str) -> Result<()> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "{message}").context("could not write command output")
}

fn preserve_draft(
    vault: &Vault,
    topic: &str,
    content: &str,
    temporary: NamedTempFile,
) -> Result<PathBuf> {
    match vault.preserve_conflict_copy(topic, content) {
        Ok(path) => Ok(path),
        Err(error) => {
            let (_, path) = temporary
                .keep()
                .context("could not preserve the draft in the vault or temporary directory")?;
            let stderr = std::io::stderr();
            let mut warning = stderr.lock();
            let _ = writeln!(
                warning,
                "warning: could not preserve a vault conflict copy ({error}); kept the temporary draft instead"
            );
            Ok(path)
        }
    }
}

fn open_editor(path: &Path) -> Result<ExitStatus> {
    let command = editor_command()?;
    let (program, arguments) = command
        .split_first()
        .expect("editor_command always returns at least one argument");

    Command::new(program)
        .args(arguments)
        .arg(path)
        .status()
        .with_context(|| format!("failed to start editor {program:?}"))
}

fn editor_command() -> Result<Vec<OsString>> {
    let configured = env::var_os("VISUAL")
        .filter(|value| !value.is_empty())
        .or_else(|| env::var_os("EDITOR").filter(|value| !value.is_empty()));

    match configured {
        Some(command) => parse_editor_command(&command, platform_editor_syntax()),
        None if cfg!(windows) => Ok(vec![OsString::from("notepad")]),
        None => Ok(vec![OsString::from("vi")]),
    }
}

const fn platform_editor_syntax() -> EditorSyntax {
    if cfg!(windows) {
        EditorSyntax::Windows
    } else {
        EditorSyntax::Unix
    }
}

fn parse_editor_command(command: &OsStr, syntax: EditorSyntax) -> Result<Vec<OsString>> {
    let Some(command) = command.to_str() else {
        return Ok(vec![command.to_os_string()]);
    };

    let parsed = match syntax {
        EditorSyntax::Unix => shell_words::split(command).map_err(|error| {
            CliFailure::EditorConfig(format!("could not parse VISUAL/EDITOR: {error}"))
        })?,
        EditorSyntax::Windows => winsplit::split(command),
    };

    if parsed.is_empty() || parsed[0].is_empty() {
        return Err(CliFailure::EditorConfig(
            "VISUAL/EDITOR does not contain an executable".to_owned(),
        )
        .into());
    }

    Ok(parsed.into_iter().map(OsString::from).collect())
}

#[cfg(test)]
mod tests {
    use super::{EditorSyntax, parse_editor_command};
    use std::ffi::{OsStr, OsString};
    #[cfg(unix)]
    use std::os::unix::ffi::OsStringExt;

    fn parse(command: &str, syntax: EditorSyntax) -> Vec<OsString> {
        parse_editor_command(OsStr::new(command), syntax).expect("valid editor command")
    }

    #[test]
    fn parses_unix_editor_commands_without_a_shell() {
        assert_eq!(
            parse("code --wait --reuse-window", EditorSyntax::Unix),
            ["code", "--wait", "--reuse-window"]
        );
        assert_eq!(
            parse(
                r#""/Applications/Visual Studio Code.app/Contents/MacOS/Electron" --wait"#,
                EditorSyntax::Unix
            ),
            [
                "/Applications/Visual Studio Code.app/Contents/MacOS/Electron",
                "--wait"
            ]
        );
    }

    #[test]
    fn rejects_unbalanced_unix_editor_quotes() {
        let error = parse_editor_command(
            OsStr::new(r#""/Applications/Visual Studio Code.app --wait"#),
            EditorSyntax::Unix,
        )
        .expect_err("unbalanced quote");
        assert!(error.to_string().contains("could not parse VISUAL/EDITOR"));
    }

    #[test]
    fn parses_windows_editor_commands_on_every_platform() {
        assert_eq!(
            parse(
                r#""C:\Program Files\Microsoft VS Code\bin\code.cmd" --wait"#,
                EditorSyntax::Windows
            ),
            [r"C:\Program Files\Microsoft VS Code\bin\code.cmd", "--wait"]
        );
        assert_eq!(
            parse(
                r#"notepad.exe "draft with spaces.md""#,
                EditorSyntax::Windows
            ),
            ["notepad.exe", "draft with spaces.md"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn accepts_a_non_unicode_unix_executable_as_one_path() {
        let executable = OsString::from_vec(b"/tmp/editor-\xff".to_vec());
        assert_eq!(
            parse_editor_command(&executable, EditorSyntax::Unix).expect("executable path"),
            [executable]
        );
    }
}
