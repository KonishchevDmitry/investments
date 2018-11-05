use chrono::Duration;

use broker_statement::{BrokerStatement, ib::IbStatementParser};
use config::Config;
use core::EmptyResult;
use currency::converter::CurrencyConverter;
use db;
use quotes::Quotes;
use types::Decimal;
use util;

use self::performance::PortfolioPerformanceAnalyser;

mod deposit_emulator;
mod performance;

pub fn analyse(config: &Config, broker_statement_path: &str) -> EmptyResult {
    let database = db::connect(&config.db_path)?;
    let converter = CurrencyConverter::new(database.clone(), false);
    let mut quotes = Quotes::new(&config, database.clone());

    let mut statement = IbStatementParser::parse(&config, broker_statement_path, false)?;
    check_statement_date(&statement);
    statement.batch_quotes(&mut quotes);
    statement.emulate_sellout(&mut quotes)?;

    for currency in ["USD", "RUB"].iter() {
        PortfolioPerformanceAnalyser::analyse(&statement, *currency, &converter)?;
    }

    Ok(())
}

fn check_statement_date(statement: &BrokerStatement) {
    let statement_date = statement.period.1 - Duration::days(1);
    let days = (util::today() - statement_date).num_days();
    let months = Decimal::from(days) / dec!(30);

    if months >= dec!(1) {
        warn!("The broker statement is {} months old and may be outdated.",
              util::round_to(months, 1));
    }
}