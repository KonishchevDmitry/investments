use crate::broker_statement::BrokerStatement;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::converter::CurrencyConverter;
use crate::quotes::Quotes;

pub fn simulate_sell(
    portfolio: &PortfolioConfig, statement: BrokerStatement,
    converter: CurrencyConverter, quotes: Quotes,
    positions: &Vec<(u32, String)>,
) -> EmptyResult {
    Err!("Not implemented yet")
}
