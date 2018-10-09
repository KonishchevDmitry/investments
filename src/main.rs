extern crate bigdecimal;
extern crate chrono;
extern crate csv;
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;
extern crate easy_logging;
#[cfg(test)] #[macro_use] extern crate indoc;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
#[cfg(test)] #[macro_use] extern crate matches;
#[cfg(test)] extern crate mockito;
extern crate reqwest;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_xml_rs;
extern crate tempfile;

#[macro_use] mod core;
mod db;
mod broker_statement;
mod currency;
mod types;
mod util;

fn main() {
    easy_logging::init(module_path!(), log::Level::Trace).unwrap();

    match broker_statement::ib::IbStatementParser::new().parse() {
        Ok(statement) => debug!("{:#?}", statement),
        Err(e) => println!("Error: {}.", e),
    }
}