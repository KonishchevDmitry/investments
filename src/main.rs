extern crate chrono;
extern crate clap;
extern crate csv;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;
extern crate easy_logging;
#[cfg(test)] #[macro_use] extern crate indoc;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
#[cfg(test)] #[macro_use] extern crate matches;
#[cfg(test)] extern crate mockito;
extern crate num_traits;
extern crate regex;
extern crate reqwest;
extern crate rust_decimal;
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
mod config;
mod currency;
mod db;
mod regulations;
mod util;

use std::process;

use config::{Config, Action};
use core::EmptyResult;

fn main() {
    let (action, config) = config::load();

    if let Err(e) = run(action, config) {
        error!("{}.", e);
        process::exit(1);
    }
}

fn run(action: Action, config: Config) -> EmptyResult {
    let database = db::connect(&config.db_path)?;

    match action {
        Action::Analyse(broker_statement_path) => analyse::analyse(database, &broker_statement_path)?,
    }

    Ok(())
}