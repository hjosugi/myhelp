mod editor;
mod output;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use dialoguer::{FuzzySelect, theme::ColorfulTheme, theme::SimpleTheme};
use myhelp_core::{Error as CoreError, Vault};
use output::{TerminalContext, write_page, write_summaries};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use thiserror::Error;

pub const EXIT_RUNTIME: u8 = 1;
pub const EXIT_USAGE: u8 = 2;
pub const EXIT_NOT_FOUND: u8 = 3;
pub const EXIT_CONFLICT: u8 = 4;
pub const EXIT_INVALID_DATA: u8 = 5;
pub const EXIT_EDITOR: u8 = 6;
pub const EXIT_CANCELLED: u8 = 130;

#[derive(Debug, Parser)]
#[command(
    name = "myhelp",
    version,
    about = "Create, search, and read personal help pages"
)]
pub struct Cli {
    /// Override the page directory (also available as MYHELP_PAGES_DIR).
    #[arg(long, global = true)]
    pages_dir: Option<PathBuf>,

    /// Control terminal styling. NO_COLOR disables color in auto mode.
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    color: ColorMode,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Default, clap::Args)]
pub(crate) struct PageOutputArgs {
    /// Print the original Markdown without terminal rendering.
    #[arg(long, conflicts_with = "json")]
    raw: bool,

    /// Print the complete page record as one JSON object.
    #[arg(long, conflicts_with = "raw")]
    json: bool,

    /// Never open the interactive pager.
    #[arg(long)]
    no_pager: bool,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List all personal help pages.
    List {
        /// Print a JSON array instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Display one page, rendered on a terminal and raw when piped.
    Show {
        topic: String,
        #[command(flatten)]
        output: PageOutputArgs,
    },
    /// Create a tldr-compatible Markdown page.
    New {
        topic: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        edit: bool,
    },
    /// Edit a page with VISUAL or EDITOR.
    Edit { topic: String },
    /// Search page names, titles, and content.
    Search {
        query: String,
        /// Print a JSON array instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Select a page with an interactive fuzzy finder, then display it.
    Pick {
        /// Seed the fuzzy finder with a query.
        query: Option<String>,
        /// Print only the selected topic for shell integration.
        #[arg(long, conflicts_with_all = ["raw", "json", "no_pager"])]
        print_topic: bool,
        #[command(flatten)]
        output: PageOutputArgs,
    },
    /// Generate a completion script for a supported shell.
    Completions { shell: Shell },
    /// Print the active page directory.
    Path,
}

#[derive(Debug, Error)]
pub(crate) enum CliFailure {
    #[error("VISUAL/EDITOR configuration is invalid: {0}")]
    EditorConfig(String),
    #[error("editor exited with {0}; the page was not changed")]
    EditorExited(String),
    #[error(
        "editor exited with {status}; the page was not changed and the draft was preserved at {}",
        path.display()
    )]
    EditorExitedWithDraft { status: String, path: PathBuf },
    #[error("pick requires an interactive stdin and stderr terminal")]
    InteractiveRequired,
    #[error("the vault contains no pages to select")]
    NoPages,
    #[error("page selection was cancelled")]
    Cancelled,
}

pub fn run(cli: Cli) -> Result<()> {
    let terminal = TerminalContext::detect(cli.color);
    let command = cli.command.unwrap_or(Commands::List { json: false });

    if let Commands::Completions { shell } = command {
        let mut command = Cli::command();
        let mut generated = Vec::new();
        generate(shell, &mut command, "myhelp", &mut generated);
        let stdout = std::io::stdout();
        stdout
            .lock()
            .write_all(&generated)
            .context("could not write generated completions")?;
        return Ok(());
    }

    let vault = match cli.pages_dir {
        Some(path) => Vault::new(path),
        None => Vault::discover()?,
    };

    match command {
        Commands::List { json } => write_summaries(&vault.list()?, json, terminal),
        Commands::Show { topic, output } => {
            let page = vault.read(&topic)?;
            write_page(&page, output, terminal)
        }
        Commands::New { topic, title, edit } => {
            let page = vault.create(&topic, title.as_deref())?;
            write_line(&format!("created {}", page.path.display()))?;
            if edit {
                editor::edit_page(&vault, &topic)?;
            }
            Ok(())
        }
        Commands::Edit { topic } => editor::edit_page(&vault, &topic),
        Commands::Search { query, json } => write_summaries(&vault.search(&query)?, json, terminal),
        Commands::Pick {
            query,
            print_topic,
            output,
        } => pick_page(&vault, query.as_deref(), print_topic, output, terminal),
        Commands::Path => write_line(&vault.root().display().to_string()),
        Commands::Completions { .. } => unreachable!("handled before vault discovery"),
    }
}

fn pick_page(
    vault: &Vault,
    query: Option<&str>,
    print_topic: bool,
    output: PageOutputArgs,
    terminal: TerminalContext,
) -> Result<()> {
    if !terminal.can_prompt() {
        return Err(CliFailure::InteractiveRequired.into());
    }

    let pages = vault.list()?;
    if pages.is_empty() {
        return Err(CliFailure::NoPages.into());
    }
    let labels = pages
        .iter()
        .map(|page| format!("{} — {}", page.topic, page.title))
        .collect::<Vec<_>>();

    let selected = if terminal.use_color {
        let theme = ColorfulTheme::default();
        fuzzy_select(&theme, &labels, query)?
    } else {
        let theme = SimpleTheme;
        fuzzy_select(&theme, &labels, query)?
    }
    .ok_or(CliFailure::Cancelled)?;
    let topic = &pages[selected].topic;

    if print_topic {
        return write_line(topic);
    }

    let page = vault.read(topic)?;
    write_page(&page, output, terminal)
}

fn fuzzy_select(
    theme: &dyn dialoguer::theme::Theme,
    labels: &[String],
    query: Option<&str>,
) -> Result<Option<usize>> {
    let mut prompt = FuzzySelect::with_theme(theme)
        .with_prompt("Page")
        .items(labels)
        .report(false)
        .vim_mode(true);
    if let Some(query) = query {
        prompt = prompt.with_initial_text(query);
    }
    prompt
        .interact_opt()
        .context("interactive page selection failed")
}

fn write_line(line: &str) -> Result<()> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    writeln!(output, "{line}").context("could not write command output")
}

pub fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == ErrorKind::BrokenPipe)
    })
}

pub fn exit_code(error: &anyhow::Error) -> u8 {
    if let Some(failure) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<CliFailure>())
    {
        return match failure {
            CliFailure::EditorConfig(_)
            | CliFailure::EditorExited(_)
            | CliFailure::EditorExitedWithDraft { .. } => EXIT_EDITOR,
            CliFailure::InteractiveRequired => EXIT_INVALID_DATA,
            CliFailure::NoPages => EXIT_NOT_FOUND,
            CliFailure::Cancelled => EXIT_CANCELLED,
        };
    }

    if let Some(error) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<CoreError>())
    {
        return match error {
            CoreError::NotFound(_) => EXIT_NOT_FOUND,
            CoreError::Conflict { .. } => EXIT_CONFLICT,
            CoreError::InvalidTopic(_)
            | CoreError::AlreadyExists(_)
            | CoreError::UnsafeSymlink(_)
            | CoreError::UnsafeFileType(_)
            | CoreError::PageTooLarge { .. }
            | CoreError::InputTooLarge { .. } => EXIT_INVALID_DATA,
            CoreError::MissingDataDirectory | CoreError::Io(_) | CoreError::WalkDir(_) => {
                EXIT_RUNTIME
            }
        };
    }

    EXIT_RUNTIME
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands, EXIT_INVALID_DATA, EXIT_NOT_FOUND, exit_code, is_broken_pipe};
    use anyhow::Error;
    use clap::{CommandFactory, Parser};
    use myhelp_core::Error as CoreError;
    use std::io::ErrorKind;

    #[test]
    fn cli_definition_is_self_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn default_command_remains_list() {
        let cli = Cli::try_parse_from(["myhelp"]).expect("parse defaults");
        assert!(cli.command.is_none());
    }

    #[test]
    fn all_completion_shells_are_accepted() {
        for shell in ["bash", "fish", "zsh", "powershell", "elvish"] {
            let cli = Cli::try_parse_from(["myhelp", "completions", shell])
                .expect("parse completion shell");
            assert!(matches!(cli.command, Some(Commands::Completions { .. })));
        }
    }

    #[test]
    fn exit_codes_distinguish_not_found_and_invalid_data() {
        assert_eq!(
            exit_code(&Error::new(CoreError::NotFound("missing".to_owned()))),
            EXIT_NOT_FOUND
        );
        assert_eq!(
            exit_code(&Error::new(CoreError::InvalidTopic("../escape".to_owned()))),
            EXIT_INVALID_DATA
        );
    }

    #[test]
    fn broken_pipe_is_a_successful_early_consumer_exit() {
        let error = Error::new(std::io::Error::new(ErrorKind::BrokenPipe, "closed"));
        assert!(is_broken_pipe(&error));
    }
}
