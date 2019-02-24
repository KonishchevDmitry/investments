use crate::broker_statement::BrokerStatement;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::converter::CurrencyConverter;
use crate::quotes::Quotes;

pub fn simulate_sell(
    portfolio: &PortfolioConfig, mut statement: BrokerStatement,
    converter: CurrencyConverter, mut quotes: Quotes,
    positions: &Vec<(String, u32)>,
) -> EmptyResult {
    for (symbol, _) in positions {
        if statement.open_positions.get(symbol).is_none() {
            return Err!("The portfolio has no open {:?} position", symbol);
        }

        quotes.batch(&symbol);
    }

    for (symbol, quantity) in positions {
        statement.emulate_sell(&symbol, *quantity, quotes.get(&symbol)?)?;
    }
    statement.process_trades()?;

    Err!("Not implemented yet")
}
