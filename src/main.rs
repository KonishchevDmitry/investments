extern crate bigdecimal;
extern crate chrono;
extern crate csv;
extern crate easy_logging;
#[macro_use] extern crate indoc;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
#[cfg(test)] extern crate mockito;
extern crate reqwest;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_xml_rs;

#[macro_use] mod core;
mod currency;
mod broker_statement;
mod types;
mod util;

fn main() {
    easy_logging::init(module_path!(), log::Level::Trace).unwrap();

    match broker_statement::ib::IbStatementParser::new().parse() {
        Ok(statement) => debug!("{:#?}", statement),
        Err(e) => println!("Error: {}.", e),
    }
}