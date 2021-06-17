use crate::broker_statement::{BrokerStatement, ReadingStrictness};
use crate::config::Config;
use crate::core::GenericResult;
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::localities::Jurisdiction;
use crate::telemetry::TelemetryRecordBuilder;

pub use self::statement::TaxStatement;

mod dividends;
mod interest;
mod statement;
mod tax_agent;
mod trades;

pub fn generate_tax_statement(
    config: &Config, portfolio_name: &str, year: Option<i32>, tax_statement_path: Option<&str>
) -> GenericResult<TelemetryRecordBuilder> {
    let country = config.get_tax_country();
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;

    let broker_statement = BrokerStatement::read(
        broker, &portfolio.statements, &portfolio.symbol_remapping, &portfolio.instrument_names,
        portfolio.get_tax_remapping()?, &portfolio.corporate_actions,
        ReadingStrictness::TRADE_SETTLE_DATE)?;

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

    let trades_tax = trades::process_income(
        &country, &portfolio, &broker_statement, year, tax_statement.as_mut(), &converter,
    ).map_err(|e| format!("Failed to process income from stock trading: {}", e))?;

    let dividends_tax = dividends::process_income(
        &country, &broker_statement, year, tax_statement.as_mut(), &converter,
    ).map_err(|e| format!("Failed to process dividend income: {}", e))?;

    let interest_tax = interest::process_income(
        &country, &broker_statement, year, tax_statement.as_mut(), &converter,
    ).map_err(|e| format!("Failed to process income from idle cash interest: {}", e))?;

    if broker_statement.broker.type_.jurisdiction() == Jurisdiction::Russia {
        let total_tax = trades_tax
            .add(dividends_tax).unwrap()
            .add(interest_tax).unwrap();
        tax_agent::process_tax_agent_withholdings(&broker_statement, year, total_tax);
    }

    if let Some(ref tax_statement) = tax_statement {
        tax_statement.save()?;
    }

    Ok(TelemetryRecordBuilder::new_with_broker(portfolio.broker))
}