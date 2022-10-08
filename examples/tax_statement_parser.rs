extern crate investments;

use std::path::PathBuf;
use std::process;

use clap::{Arg, ArgAction, Command, value_parser};

use investments::core::EmptyResult;
use investments::tax_statement::TaxStatement;

pub fn run() -> EmptyResult {
    let matches = Command::new("Tax statement parser")
        .about("Parses *.dcX file and prints its contents to stdout")
        .help_expected(true)
        .disable_help_subcommand(true)
        .args([
            Arg::new("TAX_STATEMENT")
                .help("Path to tax statement *.dcX file")
                .value_parser(value_parser!(PathBuf))
                .required(true),

            Arg::new("verbose").short('v').long("verbose")
                .help("Verbose logging")
                .action(ArgAction::SetTrue),
        ])
        .get_matches();

    if matches.get_flag("verbose") {
        easy_logging::init("investments", log::Level::Trace).map_err(|e| format!(
            "Failed to initialize the logging: {}.", e))?;
    }

    let path = matches.get_one::<PathBuf>("TAX_STATEMENT").unwrap();
    let statement = TaxStatement::read(path)?;
    println!("{:#?}", statement);

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}.", e);
        process::exit(1);
    }
}