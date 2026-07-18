use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use myhelp_core::{Error as CoreError, PageSummary, Vault};
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use tempfile::{Builder, NamedTempFile};

#[derive(Debug, Parser)]
#[command(
    name = "myhelp",
    version,
    about = "Create, search, and read personal help pages"
)]
struct Cli {
    /// Override the page directory (also available as MYHELP_PAGES_DIR).
    #[arg(long, global = true)]
    pages_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List all personal help pages.
    List,
    /// Display one page as Markdown.
    Show { topic: String },
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
    Search { query: String },
    /// Print the active page directory.
    Path,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let vault = match cli.pages_dir {
        Some(path) => Vault::new(path),
        None => Vault::discover()?,
    };

    match cli.command.unwrap_or(Commands::List) {
        Commands::List => print_summaries(&vault.list()?),
        Commands::Show { topic } => print!("{}", vault.read(&topic)?.content),
        Commands::New { topic, title, edit } => {
            let page = vault.create(&topic, title.as_deref())?;
            println!("created {}", page.path.display());
            if edit {
                edit_page(&vault, &topic)?;
            }
        }
        Commands::Edit { topic } => edit_page(&vault, &topic)?,
        Commands::Search { query } => print_summaries(&vault.search(&query)?),
        Commands::Path => println!("{}", vault.root().display()),
    }

    Ok(())
}

fn print_summaries(pages: &[PageSummary]) {
    for page in pages {
        println!("{:<28} {}", page.topic, page.title);
    }
}

fn edit_page(vault: &Vault, topic: &str) -> Result<()> {
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
            bail!("editor exited with {status}; the page was not changed");
        }
        let preserved = preserve_draft(vault, topic, &edited, temporary)?;
        bail!(
            "editor exited with {status}; the page was not changed and the draft was preserved at {}",
            preserved.display()
        );
    }

    if edited == page.content {
        println!("unchanged {}", page.path.display());
        return Ok(());
    }

    match vault.save(topic, &edited, &page.revision) {
        Ok(saved) => {
            println!("saved {}", saved.path.display());
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
            eprintln!(
                "warning: could not preserve a vault conflict copy ({error}); kept the temporary draft instead"
            );
            Ok(path)
        }
    }
}

fn open_editor(path: &std::path::Path) -> Result<ExitStatus> {
    let editor = env::var_os("VISUAL")
        .or_else(|| env::var_os("EDITOR"))
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "notepad".into()
            } else {
                "vi".into()
            }
        });

    let status = Command::new(&editor)
        .arg(path)
        .status()
        .with_context(|| format!("failed to start editor {:?}", editor))?;
    Ok(status)
}
