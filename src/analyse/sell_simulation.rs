use crate::broker_statement::BrokerStatement;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::converter::CurrencyConverter;
use crate::quotes::Quotes;

pub fn simulate_sell(
    portfolio: &PortfolioConfig, statement: BrokerStatement,
    converter: CurrencyConverter, mut quotes: Quotes,
    positions: &Vec<(String, u32)>,
) -> EmptyResult {
    for (symbol, _) in positions {
        if let None = statement.open_positions.get(symbol) {
            return Err!("The portfolio has no open {:?} position", symbol);
        }

        quotes.batch(&symbol);
    }

    Err!("Not implemented yet")
}
