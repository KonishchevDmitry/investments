use broker_statement::ib::IbStatementParser;
use config::Config;
use core::EmptyResult;
use currency::CashAssets;
use currency::converter::CurrencyConverter;
use db;

mod deposit_emulator;
pub mod performance;

pub fn analyse(config: &Config, broker_statement_path: &str) -> EmptyResult {
    let database = db::connect(&config.db_path)?;
    let statement = IbStatementParser::parse(&config, broker_statement_path)?;
    let converter = CurrencyConverter::new(database, false);
    let total_value = CashAssets::new_from_cash(statement.period.1, statement.total_value);

    println!("Average rate of return from cash investments:");

    for currency in ["USD", "RUB"].iter() {
        let interest = performance::get_average_rate_of_return(
            &statement, total_value, *currency, &converter)?;

        println!("{}: {}", currency, interest);
    }

    Ok(())
}