extern crate investments;

use std::process;

use investments::cli;
use investments::core::EmptyResult;
use investments::tax_statement::TaxStatement;

pub fn run() -> EmptyResult {
    let matches =
        cli::new_app("Tax statement parser", "Parses *.dcX file and prints its contents to stdout")
        .args([
            cli::new_arg("TAX_STATEMENT", "Path to tax statement *.dcX file").required(true),
            cli::new_arg("verbose", "Verbose logging").short('v').long("verbose"),
        ])
        .get_matches();

    if matches.is_present("verbose") {
        easy_logging::init("investments", log::Level::Trace).map_err(|e| format!(
            "Failed to initialize the logging: {}.", e))?;
    }

    let path = matches.value_of("TAX_STATEMENT").unwrap();
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