use broker_statement::ib::IbStatementParser;
use config::Config;
use core::EmptyResult;
use currency::converter::CurrencyConverter;
use db;

mod deposit_emulator;
pub mod performance;

pub fn analyse(config: &Config, broker_statement_path: &str) -> EmptyResult {
    let database = db::connect(&config.db_path)?;
    let statement = IbStatementParser::parse(&config, broker_statement_path, false)?;
    let converter = CurrencyConverter::new(database, false);

    println!("Average rate of return from cash investments:");

    for currency in ["USD", "RUB"].iter() {
        let interest = performance::AverageRateOfReturnCalculator::calculate(
            &statement, *currency, &converter)?;
        println!("{}: {}", currency, interest);
    }

    Ok(())
}