extern crate chrono;
extern crate chrono_tz;
extern crate clap;
extern crate csv;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;
extern crate easy_logging;
extern crate encoding_rs;
#[cfg(test)] #[macro_use] extern crate indoc;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
#[cfg(test)] #[macro_use] extern crate matches;
#[cfg(test)] extern crate mockito;
extern crate num_traits;
extern crate prettytable;
extern crate regex;
extern crate reqwest;
extern crate rust_decimal;
extern crate separator;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_xml_rs;
extern crate serde_yaml;
extern crate shellexpand;
extern crate tempfile;

#[macro_use] mod core;
#[macro_use] mod types;
mod analyse;
mod broker_statement;
mod brokers;
mod config;
mod currency;
mod db;
mod formatting;
mod init;
mod quotes;
mod regulations;
mod tax_statement;
mod util;

use std::process;

use config::Config;
use core::EmptyResult;
use init::{Action, initialize};

fn main() {
    let (action, config) = initialize();

    if let Err(e) = run(action, config) {
        error!("{}.", e);
        process::exit(1);
    }
}

fn run(action: Action, config: Config) -> EmptyResult {
    match action {
        Action::Analyse(portfolio_name) =>
            analyse::analyse(&config, &portfolio_name)?,

        Action::TaxStatement { portfolio_name, year, tax_statement_path } =>
            tax_statement::generate_tax_statement(
                &config, &portfolio_name, year, tax_statement_path.as_ref().map(String::as_str))?,
    };

    Ok(())
}