use clap::Parser;
use myhelp_cli::i18n::{Locale, error_line};
use myhelp_cli::{Cli, exit_code, is_broken_pipe, run_with_locale};
use std::process::ExitCode;

fn main() -> ExitCode {
    let locale = Locale::detect();
    match run_with_locale(Cli::parse(), locale) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if is_broken_pipe(&error) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", error_line(&error, locale));
            ExitCode::from(exit_code(&error))
        }
    }
}
