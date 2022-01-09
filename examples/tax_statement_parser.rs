extern crate investments;

use std::process;

use investments::cli;
use investments::core::EmptyResult;
use investments::tax_statement::TaxStatement;

pub fn run() -> EmptyResult {
    let matches =
        cli::new_app("Tax statement parser", "Parses *.dcX file and prints its contents to stdout")
        .arg(cli::new_arg("TAX_STATEMENT", "Path to tax statement *.dcX file").required(true))
        .get_matches();

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