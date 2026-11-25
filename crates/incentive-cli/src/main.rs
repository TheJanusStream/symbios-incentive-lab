//! `incentive`: the incentive lab's command-line runner.

/// Writes formatted text to stdout; a closed pipe becomes an error, not a panic.
macro_rules! out {
    ($($arg:tt)*) => {
        crate::report::emit(&format!($($arg)*))?
    };
}

/// [`out!`] with a newline.
macro_rules! outln {
    ($($arg:tt)*) => {
        crate::report::emit(&format!("{}\n", format!($($arg)*)))?
    };
}

mod cli;
mod commands;
mod error;
mod explore;
mod facts_cmd;
mod listing;
mod report;
mod runs;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::error::CliError;

fn main() -> ExitCode {
    match dispatch(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Stdout(e)) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Command::Presets => listing::list_presets()?,
        Command::Scenarios => listing::list_scenarios()?,
        Command::Levers => listing::list_levers()?,
        Command::Config { preset } => listing::print_config(&preset)?,
        Command::Run { source, options } => commands::run(&source, &options)?,
        Command::Compare {
            a,
            b,
            set_a,
            set_b,
            options,
        } => commands::compare(&a, &b, &set_a, &set_b, &options)?,
        Command::Sweep {
            source,
            lever,
            values,
            options,
        } => commands::sweep(&source, &lever, &values, &options)?,
        Command::Scenario {
            id,
            source,
            solution,
            options,
        } => commands::scenario_cmd(&id, &source, solution, &options)?,
        Command::Explore { mode } => explore::explore(&mode)?,
        Command::Facts {
            seeds,
            population,
            days,
            out,
        } => facts_cmd::run_facts(seeds, population, days, out)?,
    }
    Ok(())
}
