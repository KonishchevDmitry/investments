extern crate bigdecimal;
extern crate chrono;
extern crate csv;
extern crate easy_logging;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
extern crate serde;
#[macro_use] extern crate serde_derive;

#[macro_use] mod core;
mod currency;
mod statement;
mod types;

fn main() {
    easy_logging::init(module_path!(), log::Level::Trace).unwrap();

    match statement::ib::IbStatementParser::new().parse() {
        Ok(statement) => debug!("{:#?}", statement),
        Err(e) => println!("Error: {}.", e),
    }
}