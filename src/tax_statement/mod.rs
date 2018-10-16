use chrono::{self, Datelike, Duration};

use broker_statement::ib::IbStatementParser;
use core::EmptyResult;
use currency::CashAssets;
use currency::converter::CurrencyConverter;
use db;
use types::Date;
use util;

pub fn generate_tax_statement(
    database: db::Connection, year: i32,
    broker_statement_path: &str, tax_statement_path: Option<&str>
) -> EmptyResult {
    let broker_statement = IbStatementParser::parse(broker_statement_path)?;
    let converter = CurrencyConverter::new(database);

    if year > chrono::Local::today().year() {
        return Err!("An attempt to generate tax statement for the future");
    }

    let tax_period_start = date!(1, 1, year);
    let tax_period_end = date!(1, 1, year + 1);

    if tax_period_start >= broker_statement.period.0 && tax_period_end <= broker_statement.period.1 {
        // Broker statement period more or equal to the tax year period
    } else if tax_period_end > broker_statement.period.0 && tax_period_start < broker_statement.period.1 {
        warn!(concat!(
            "Period of the specified broker statement ({} - {}) ",
            "doesn't fully overlap with the requested tax year ({})."),
            util::format_date(broker_statement.period.0),
            util::format_date(broker_statement.period.1 - Duration::days(1)), year);
    } else {
        return Err!(concat!(
            "Period of the specified broker statement ({} - {}) ",
            "doesn't overlap with the requested tax year ({})"),
            util::format_date(broker_statement.period.0),
            util::format_date(broker_statement.period.1 - Duration::days(1)), year);
    }

    Ok(())
}