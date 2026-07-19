mod editor;
mod output;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use dialoguer::{FuzzySelect, theme::ColorfulTheme, theme::SimpleTheme};
use myhelp_core::{
    AdapterCompatibility, AdapterConversionReport, AdapterDiagnostic, AdapterDiagnosticLevel,
    AdapterDisposition, Error as CoreError, TldrDiagnostic, TldrDiagnosticLevel, TldrImportOptions,
    TldrSource, TldrValidation, Vault, inspect_navi_file, validate_tldr_file,
};
use output::{TerminalContext, write_page, write_summaries};
use serde::Serialize;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
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
    /// Validate, import, or export tldr/tealdeer pages.
    Tldr {
        #[command(subcommand)]
        command: TldrCommands,
    },
    /// Inspect foreign formats without executing commands or changing the vault.
    Adapter {
        #[command(subcommand)]
        command: AdapterCommands,
    },
    /// Print the active page directory.
    Path,
}

#[derive(Debug, Subcommand)]
enum TldrCommands {
    /// Validate one tldr-style Markdown file.
    Validate {
        path: PathBuf,
        /// Expected topic when it cannot be derived from the filename.
        #[arg(long)]
        topic: Option<String>,
        /// Print the validation report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Import a flat tldr or tealdeer custom page without rewriting it.
    Import {
        path: PathBuf,
        /// Destination topic; may contain nested categories.
        #[arg(long)]
        topic: Option<String>,
        /// Page license value, such as an SPDX expression or LicenseRef.
        #[arg(long)]
        page_license: Option<String>,
        /// Absolute provenance URL stored in a new metadata sidecar.
        #[arg(long)]
        source_url: Option<String>,
        /// Human-readable source name.
        #[arg(long, requires = "source_url")]
        source_title: Option<String>,
        /// Source license value, such as an SPDX expression or LicenseRef.
        #[arg(long, requires = "source_url")]
        source_license: Option<String>,
        /// Attribution text retained with the imported content.
        #[arg(long, requires = "source_url")]
        attribution: Option<String>,
        /// Print the conversion report as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Export every vault page to a collision-safe flat directory.
    Export {
        destination: PathBuf,
        /// Print the deterministic mapping as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum AdapterCommands {
    /// Generate a dry-run conversion report for one foreign-format file.
    Inspect {
        /// Source format. Navi is the first implemented prototype.
        #[arg(value_enum)]
        format: AdapterFormat,
        path: PathBuf,
        /// Proposed MyHelp topic; defaults to the source filename.
        #[arg(long)]
        topic: Option<String>,
        /// Print the complete conversion report as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum AdapterFormat {
    Navi,
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
    #[error("tldr validation failed for {}", .0.display())]
    TldrValidationFailed(PathBuf),
    #[error("adapter conversion is not safe for import: {}", .0.display())]
    AdapterConversionFailed(PathBuf),
}

pub fn run(cli: Cli) -> Result<()> {
    let terminal = TerminalContext::detect(cli.color);
    let command = cli.command.unwrap_or(Commands::List { json: false });

    match &command {
        Commands::Completions { shell } => {
            let mut command = Cli::command();
            let mut generated = Vec::new();
            generate(*shell, &mut command, "myhelp", &mut generated);
            let stdout = std::io::stdout();
            stdout
                .lock()
                .write_all(&generated)
                .context("could not write generated completions")?;
            return Ok(());
        }
        Commands::Tldr {
            command: TldrCommands::Validate { path, topic, json },
        } => return validate_tldr(path, topic.as_deref(), *json),
        Commands::Adapter {
            command:
                AdapterCommands::Inspect {
                    format,
                    path,
                    topic,
                    json,
                },
        } => return inspect_adapter(*format, path, topic.as_deref(), *json),
        _ => {}
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
        Commands::Tldr { command } => run_tldr(&vault, command),
        Commands::Adapter { .. } => unreachable!("handled before vault discovery"),
        Commands::Path => write_line(&vault.root().display().to_string()),
        Commands::Completions { .. } => unreachable!("handled before vault discovery"),
    }
}

fn inspect_adapter(
    format: AdapterFormat,
    path: &Path,
    topic: Option<&str>,
    json: bool,
) -> Result<()> {
    let report = match format {
        AdapterFormat::Navi => inspect_navi_file(path, topic)?,
    };
    write_adapter_report(&report, json)?;
    if report.convertible {
        Ok(())
    } else {
        Err(CliFailure::AdapterConversionFailed(path.to_path_buf()).into())
    }
}

fn write_adapter_report(report: &AdapterConversionReport, json: bool) -> Result<()> {
    if json {
        return write_json(report);
    }

    write_line(&format!(
        "{} -> {} (dry-run, {})",
        report.source_path.display(),
        report.topic,
        compatibility_label(report.compatibility)
    ))?;
    write_line(if report.convertible {
        "status: convertible with the reported losses"
    } else {
        "status: not convertible without resolving errors"
    })?;
    if !report.source_tags.is_empty() {
        write_line(&format!("source tags: {}", report.source_tags.join(", ")))?;
    }
    write_adapter_diagnostics(&report.source_path, &report.diagnostics)?;
    if let Some(page) = &report.generated_page {
        write_line("generated page preview:")?;
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        output
            .write_all(page.as_bytes())
            .context("could not write command output")?;
    }
    Ok(())
}

fn write_adapter_diagnostics(path: &Path, diagnostics: &[AdapterDiagnostic]) -> Result<()> {
    for diagnostic in diagnostics {
        let level = match diagnostic.level {
            AdapterDiagnosticLevel::Info => "info",
            AdapterDiagnosticLevel::Warning => "warning",
            AdapterDiagnosticLevel::Error => "error",
        };
        let location = diagnostic.line.map_or_else(
            || path.display().to_string(),
            |line| format!("{}:{line}", path.display()),
        );
        write_line(&format!(
            "{location}: {level}[{}] {} ({}): {}",
            diagnostic.code,
            diagnostic.source_field,
            disposition_label(diagnostic.disposition),
            diagnostic.message
        ))?;
    }
    Ok(())
}

fn compatibility_label(compatibility: AdapterCompatibility) -> &'static str {
    match compatibility {
        AdapterCompatibility::LossyImportPreview => "lossy import preview",
        AdapterCompatibility::ReadOnlyIndex => "read-only index",
        AdapterCompatibility::Unsupported => "unsupported",
    }
}

fn disposition_label(disposition: AdapterDisposition) -> &'static str {
    match disposition {
        AdapterDisposition::Mapped => "mapped",
        AdapterDisposition::ReportedOnly => "reported only",
        AdapterDisposition::Unsupported => "unsupported",
    }
}

fn validate_tldr(path: &Path, topic: Option<&str>, json: bool) -> Result<()> {
    let validation = validate_tldr_file(path, topic)?;
    write_validation(path, &validation, json)?;
    if validation.valid {
        Ok(())
    } else {
        Err(CliFailure::TldrValidationFailed(path.to_path_buf()).into())
    }
}

fn run_tldr(vault: &Vault, command: TldrCommands) -> Result<()> {
    match command {
        TldrCommands::Validate { .. } => unreachable!("handled before vault discovery"),
        TldrCommands::Import {
            path,
            topic,
            page_license,
            source_url,
            source_title,
            source_license,
            attribution,
            json,
        } => {
            let source = source_url.map(|url| TldrSource {
                url,
                title: source_title,
                license: source_license,
                attribution,
            });
            let options = TldrImportOptions {
                topic,
                page_license,
                source,
            };
            match vault.import_tldr_page(&path, &options) {
                Ok(report) => {
                    if json {
                        write_json(&report)
                    } else {
                        write_line(&format!(
                            "{} -> {}",
                            path.display(),
                            report.page_path.display()
                        ))?;
                        if let Some(metadata) = report.metadata_path {
                            write_line(&format!("metadata -> {}", metadata.display()))?;
                        }
                        write_diagnostics(&path, &report.validation.diagnostics)
                    }
                }
                Err(error) => report_tldr_failure(error, json),
            }
        }
        TldrCommands::Export { destination, json } => match vault.export_tldr_pages(&destination) {
            Ok(report) => {
                if json {
                    write_json(&report)
                } else {
                    for mapping in &report.mappings {
                        let suffix = if mapping.collision_resolved {
                            " (collision resolved)"
                        } else {
                            ""
                        };
                        write_line(&format!(
                            "{} -> {}{suffix}",
                            mapping.topic, mapping.page_file
                        ))?;
                        write_diagnostics(
                            Path::new(&mapping.topic),
                            &mapping.validation.diagnostics,
                        )?;
                    }
                    write_line(&format!(
                        "exported {} page(s) to {}",
                        report.mappings.len(),
                        report.destination.display()
                    ))
                }
            }
            Err(error) => report_tldr_failure(error, json),
        },
    }
}

fn report_tldr_failure(error: CoreError, json: bool) -> Result<()> {
    if let CoreError::InvalidTldr { path, diagnostics } = &error {
        let validation = TldrValidation {
            valid: false,
            diagnostics: diagnostics.clone(),
        };
        write_validation(path, &validation, json)?;
    }
    Err(error.into())
}

fn write_validation(path: &Path, validation: &TldrValidation, json: bool) -> Result<()> {
    if json {
        return write_json(validation);
    }
    write_diagnostics(path, &validation.diagnostics)?;
    if validation.diagnostics.is_empty() {
        write_line(&format!("{}: valid tldr page", path.display()))?;
    }
    Ok(())
}

fn write_diagnostics(path: &Path, diagnostics: &[TldrDiagnostic]) -> Result<()> {
    for diagnostic in diagnostics {
        let level = match diagnostic.level {
            TldrDiagnosticLevel::Error => "error",
            TldrDiagnosticLevel::Warning => "warning",
        };
        write_line(&format!(
            "{}:{}: {level}[{}]: {}",
            path.display(),
            diagnostic.line,
            diagnostic.code,
            diagnostic.message
        ))?;
    }
    Ok(())
}

fn write_json(value: &impl Serialize) -> Result<()> {
    let serialized = serde_json::to_vec(value).context("could not serialize command output")?;
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    output
        .write_all(&serialized)
        .context("could not write command output")?;
    output
        .write_all(b"\n")
        .context("could not write command output")
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
            CliFailure::TldrValidationFailed(_) | CliFailure::AdapterConversionFailed(_) => {
                EXIT_INVALID_DATA
            }
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
            | CoreError::InvalidTldr { .. }
            | CoreError::InvalidImportMetadata(_)
            | CoreError::ExportDestinationOccupied(_)
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
