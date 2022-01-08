extern crate investments;

use std::process;

use clap::{App, Arg, AppSettings};

use investments::core::EmptyResult;
use investments::tax_statement::TaxStatement;

pub fn run() -> EmptyResult {
    let matches = App::new("Tax statement parser")
        .about("\nParses *.dcX file and prints its contents to stdout")
        .arg(Arg::new("TAX_STATEMENT")
            .help("Path to tax statement *.dcX file")
            .required(true))
        .global_setting(AppSettings::DisableVersionFlag)
        .global_setting(AppSettings::DeriveDisplayOrder)
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