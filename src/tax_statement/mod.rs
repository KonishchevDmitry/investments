use chrono::Datelike;
use log::warn;

use crate::broker_statement::BrokerStatement;
use crate::config::Config;
use crate::core::EmptyResult;
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::formatting;
use crate::util;

pub use self::statement::TaxStatement;

mod dividends;
mod interest;
mod statement;
mod trades;

pub fn generate_tax_statement(
    config: &Config, portfolio_name: &str, year: Option<i32>, tax_statement_path: Option<&str>
) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker_statement = BrokerStatement::read(
        config, portfolio.broker, &portfolio.statements, true)?;

    if let Some(year) = year {
        if year > util::today().year() {
            return Err!("An attempt to generate tax statement for the future");
        }

        validate_broker_statement_period(&broker_statement, year)?;
    }

    let mut tax_statement = match tax_statement_path {
        Some(path) => {
            let year = year.ok_or_else(||
                "Tax year must be specified when tax statement is specified")?;

            let statement = TaxStatement::read(path)?;
            if statement.year != year {
                return Err!("Tax statement year ({}) doesn't match the requested year {}",
                            statement.year, year);
            }

            Some(statement)
        },
        None => None,
    };

    let database = db::connect(&config.db_path)?;
    let converter = CurrencyConverter::new(database, None, true);

    trades::process_income(&portfolio, &broker_statement, year, tax_statement.as_mut(), &converter)
        .map_err(|e| format!("Failed to process income from stock trading: {}", e))?;

    dividends::process_income(&portfolio, &broker_statement, year, tax_statement.as_mut(), &converter)
        .map_err(|e| format!("Failed to process dividend income: {}", e))?;

    interest::process_income(&portfolio, &broker_statement, year, tax_statement.as_mut(), &converter)
        .map_err(|e| format!("Failed to process income from idle cash interest: {}", e))?;

    if let Some(ref tax_statement) = tax_statement {
        tax_statement.save()?;
    }

    Ok(())
}

fn validate_broker_statement_period(broker_statement: &BrokerStatement, year: i32) -> EmptyResult {
    let tax_period_start = date!(1, 1, year);
    let tax_period_end = date!(1, 1, year + 1);

    if tax_period_start >= broker_statement.period.0 && tax_period_end <= broker_statement.period.1 {
        // Broker statement period more or equal to the tax year period
    } else if tax_period_end > broker_statement.period.0 && tax_period_start < broker_statement.period.1 {
        warn!(concat!(
            "Period of the specified broker statement ({}) ",
            "doesn't fully overlap with the requested tax year ({})."),
              formatting::format_period(broker_statement.period.0, broker_statement.period.1), year);
    } else {
        return Err!(concat!(
            "Period of the specified broker statement ({}) ",
            "doesn't overlap with the requested tax year ({})"),
            formatting::format_period(broker_statement.period.0, broker_statement.period.1), year);
    }

    Ok(())
}