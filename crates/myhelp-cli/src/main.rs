use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use myhelp_core::{PageSummary, Vault};
use std::env;
use std::path::PathBuf;
use std::process::Command;

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
                open_editor(&page.path)?;
            }
        }
        Commands::Edit { topic } => {
            let page = vault.read(&topic)?;
            open_editor(&page.path)?;
        }
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

fn open_editor(path: &std::path::Path) -> Result<()> {
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
    if !status.success() {
        bail!("editor exited with {status}");
    }

    Ok(())
}
