use crate::broker_statement::BrokerStatement;
use crate::config::Config;
use crate::core::EmptyResult;
use crate::currency::converter::CurrencyConverter;
use crate::db;

pub use self::statement::TaxStatement;

mod dividends;
mod interest;
mod statement;
mod trades;

pub fn generate_tax_statement(
    config: &Config, portfolio_name: &str, year: Option<i32>, tax_statement_path: Option<&str>
) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;

    let broker_statement = BrokerStatement::read(
        broker, &portfolio.statements, &portfolio.symbol_remapping, &portfolio.instrument_names,
        portfolio.get_tax_remapping()?, true)?;

    if let Some(year) = year {
        broker_statement.check_period_against_tax_year(year)?;
    }

    let mut tax_statement = match tax_statement_path {
        Some(path) => {
            let year = year.ok_or("Tax year must be specified when tax statement is specified")?;

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