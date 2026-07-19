use anyhow::{Context, Result};
use dialoguer::console::Term;
use minus::Pager;
use myhelp_core::{Page, PageSummary};
use std::borrow::Cow;
use std::env;
use std::io::{IsTerminal, Write};
use termimad::MadSkin;

use crate::{ColorMode, PageOutputArgs};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TerminalContext {
    pub(crate) stdin_is_terminal: bool,
    pub(crate) stdout_is_terminal: bool,
    pub(crate) stderr_is_terminal: bool,
    pub(crate) use_color: bool,
    pub(crate) width: usize,
}

impl TerminalContext {
    pub(crate) fn detect(color: ColorMode) -> Self {
        let stdin_is_terminal = std::io::stdin().is_terminal();
        let stdout_is_terminal = std::io::stdout().is_terminal();
        let stderr_is_terminal = std::io::stderr().is_terminal();
        let no_color = env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        let use_color = match color {
            ColorMode::Auto => stdout_is_terminal && !no_color,
            ColorMode::Always => true,
            ColorMode::Never => false,
        };
        let (_, columns) = Term::stdout().size();

        Self {
            stdin_is_terminal,
            stdout_is_terminal,
            stderr_is_terminal,
            use_color,
            width: usize::from(columns).clamp(20, 160),
        }
    }

    pub(crate) const fn can_prompt(self) -> bool {
        self.stdin_is_terminal && self.stderr_is_terminal
    }
}

pub(crate) fn write_summaries(
    pages: &[PageSummary],
    json: bool,
    terminal: TerminalContext,
) -> Result<()> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();

    if json {
        let serialized = serde_json::to_vec(pages).context("could not serialize page summaries")?;
        output
            .write_all(&serialized)
            .context("could not write command output")?;
        output
            .write_all(b"\n")
            .context("could not write command output")?;
        return Ok(());
    }

    let topic_width = pages
        .iter()
        .map(|page| page.topic.chars().count())
        .max()
        .unwrap_or_default()
        .min(40);

    for page in pages {
        let topic = sanitize_terminal_control(&page.topic);
        let title = sanitize_terminal_control(&page.title);
        if terminal.stdout_is_terminal {
            if terminal.use_color {
                writeln!(output, "\x1b[36m{topic:<topic_width$}\x1b[0m  {title}")
                    .context("could not write command output")?;
            } else {
                writeln!(output, "{topic:<topic_width$}  {title}")
                    .context("could not write command output")?;
            }
        } else if terminal.use_color {
            writeln!(output, "\x1b[36m{topic}\x1b[0m\t{title}")
                .context("could not write command output")?;
        } else {
            writeln!(output, "{topic}\t{title}").context("could not write command output")?;
        }
    }

    Ok(())
}

pub(crate) fn write_page(
    page: &Page,
    options: PageOutputArgs,
    terminal: TerminalContext,
) -> Result<()> {
    if options.json {
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        let serialized = serde_json::to_vec(page).context("could not serialize the page")?;
        output
            .write_all(&serialized)
            .context("could not write command output")?;
        output
            .write_all(b"\n")
            .context("could not write command output")?;
        return Ok(());
    }

    let rendered = !options.raw && (terminal.stdout_is_terminal || terminal.use_color);
    let content = if rendered {
        render_markdown(&page.content, terminal.use_color, terminal.width)
    } else {
        page.content.clone()
    };
    write_display(content, !options.no_pager, rendered, terminal)
}

fn write_display(
    mut content: String,
    allow_pager: bool,
    ensure_trailing_newline: bool,
    terminal: TerminalContext,
) -> Result<()> {
    if ensure_trailing_newline && !content.ends_with('\n') {
        content.push('\n');
    }

    if allow_pager && terminal.stdin_is_terminal && terminal.stdout_is_terminal {
        let pager = Pager::new();
        pager
            .set_text(content)
            .context("could not initialize the terminal pager")?;
        minus::page_all(pager).context("terminal pager failed")?;
        return Ok(());
    }

    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    output
        .write_all(content.as_bytes())
        .context("could not write command output")
}

fn render_markdown(markdown: &str, color: bool, width: usize) -> String {
    let sanitized = sanitize_terminal_control(markdown);
    let skin = if color {
        MadSkin::default()
    } else {
        MadSkin::no_style()
    };
    skin.text(&sanitized, Some(width)).to_string()
}

fn sanitize_terminal_control(value: &str) -> Cow<'_, str> {
    if value
        .chars()
        .all(|character| !character.is_control() || matches!(character, '\n' | '\t'))
    {
        return Cow::Borrowed(value);
    }

    let mut sanitized = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' && characters.peek() == Some(&'\n') {
            continue;
        }
        if !character.is_control() || matches!(character, '\n' | '\t') {
            sanitized.push(character);
        } else {
            sanitized.push('\u{fffd}');
        }
    }
    Cow::Owned(sanitized)
}

#[cfg(test)]
mod tests {
    use super::{TerminalContext, render_markdown, sanitize_terminal_control};

    #[test]
    fn no_color_rendering_has_no_ansi_sequences() {
        let rendered = render_markdown(
            "# Git rebase\n\n> Keep commits tidy.\n\n- Continue:\n\n`git rebase --continue`\n",
            false,
            80,
        );
        assert!(!rendered.contains("\u{1b}["));
        assert!(rendered.contains("git rebase --continue"));
        assert!(!rendered.contains('`'));
    }

    #[test]
    fn terminal_rendering_replaces_embedded_control_sequences() {
        let sanitized = sanitize_terminal_control("safe\u{1b}[2Jtext\r\n");
        assert_eq!(sanitized, "safe�[2Jtext\n");
    }

    #[test]
    fn prompt_requires_input_and_error_terminals_only() {
        let context = TerminalContext {
            stdin_is_terminal: true,
            stdout_is_terminal: false,
            stderr_is_terminal: true,
            use_color: false,
            width: 80,
        };
        assert!(context.can_prompt(), "stdout may be captured by a shell");
    }
}
