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
extern crate rust_decimal;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_xml_rs;
extern crate tempfile;

#[macro_use] mod core;
#[macro_use] mod types;
mod util;
mod db;
mod broker_statement;
mod currency;
mod analyse;

use currency::CashAssets;
use currency::converter::CurrencyConverter;

fn main() {
    easy_logging::init(module_path!(), log::Level::Trace).unwrap();

    let connection = db::connect("db.sqlite").unwrap();
    let converter = CurrencyConverter::new(connection);

    match broker_statement::ib::IbStatementParser::new().parse() {
        Ok(statement) => {
            debug!("{:#?}", statement);

            let total_value = CashAssets::new_from_cash(statement.period.1, statement.total_value);

            for currency in ["USD", "RUB"].iter() {
                let interest = analyse::profit::get_average_profit(
                    &statement.deposits, total_value, *currency, &converter).unwrap();

                println!("{}: {}", currency, interest * dec!(100));
            }
        },
        Err(e) => println!("Error: {}.", e),
    }
}