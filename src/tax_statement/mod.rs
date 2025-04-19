mod dividends;
mod interest;
mod statement;
mod tax_agent;
mod trades;

use std::path::Path;

use ansi_term::Color;

use crate::broker_statement::{BrokerStatement, ReadingStrictness};
use crate::config::Config;
use crate::core::GenericResult;
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::localities::Jurisdiction;
use crate::taxes::TaxCalculator;
use crate::telemetry::TelemetryRecordBuilder;

pub use self::statement::TaxStatement;

pub fn generate_tax_statement(
    config: &Config, portfolio_name: &str, year: Option<i32>, tax_statement_path: Option<&Path>
) -> GenericResult<TelemetryRecordBuilder> {
    let country = config.get_tax_country();
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_deref())?;

    let broker_statement = BrokerStatement::read(
        broker, portfolio.statements_path()?, &portfolio.symbol_remapping, &portfolio.instrument_internal_ids,
        &portfolio.instrument_names, portfolio.get_tax_remapping()?, &portfolio.tax_exemptions, &portfolio.corporate_actions,
        ReadingStrictness::TRADE_SETTLE_DATE | ReadingStrictness::OTC_INSTRUMENTS | ReadingStrictness::TAX_EXEMPTIONS |
        ReadingStrictness::REPO_TRADES | ReadingStrictness::GRANTS)?;

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
    let mut tax_calculator = TaxCalculator::new(country.clone());

    let (trades_tax, has_trading_income, has_trading_income_to_declare) = trades::process_income(
        &country, portfolio, &broker_statement, year, &mut tax_calculator, tax_statement.as_mut(), &converter,
    ).map_err(|e| format!("Failed to process income from stock trading: {}", e))?;

    let (dividends_tax, has_dividend_income, has_dividend_income_to_declare) = dividends::process_income(
        &country, &broker_statement, year, &mut tax_calculator, tax_statement.as_mut(), &converter,
    ).map_err(|e| format!("Failed to process dividend income: {}", e))?;

    let (interest_tax, has_interest_income, has_interest_income_to_declare) = interest::process_income(
        &country, &broker_statement, year, &mut tax_calculator, tax_statement.as_mut(), &converter,
    ).map_err(|e| format!("Failed to process income from idle cash interest: {}", e))?;

    let has_income = has_trading_income | has_dividend_income | has_interest_income;
    let has_income_to_declare = has_trading_income_to_declare | has_dividend_income_to_declare | has_interest_income_to_declare;

    if broker_statement.broker.type_.jurisdiction() == Jurisdiction::Russia {
        let total_tax = trades_tax + dividends_tax + interest_tax;
        tax_agent::process_tax_agent_withholdings(&broker_statement, year, has_income, total_tax)?;
    }

    if let Some(ref tax_statement) = tax_statement {
        assert_eq!(tax_statement.modified, has_income_to_declare);

        if has_income_to_declare {
            tax_statement.save()?;
            println!("{}", Color::Green.paint(
                "The income has been added to the tax statement."));
        }
    } else if has_income_to_declare {
        println!("{}", Color::Yellow.paint(
            "The income must be declared to tax inspection."));
    }

    if !has_income_to_declare {
        println!("{}", Color::Green.paint(
            "There is no any income to declare."));
    }

    Ok(TelemetryRecordBuilder::new_with_broker(portfolio.broker))
}