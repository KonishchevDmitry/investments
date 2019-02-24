use prettytable::{Table, Row, Cell};
use prettytable::format::Alignment;

use crate::broker_statement::BrokerStatement;
use crate::broker_statement::trades::StockSell;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
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

    let mut stock_sells = statement.stock_sells.iter()
        .filter(|stock_sell| stock_sell.emulation)
        .cloned().collect::<Vec<_>>();
    assert_eq!(stock_sells.len(), positions.len());

    print_results(stock_sells)
}

fn print_results(stock_sells: Vec<StockSell>) -> EmptyResult {
    let mut table = Table::new();
//    let mut fifo_details = Vec::new();

    for trade in stock_sells {
        let mut details = Table::new();



        table.add_row(Row::new(vec![
            Cell::new(&trade.symbol),
            Cell::new_align(&trade.quantity.to_string(), Alignment::RIGHT),
            formatting::cash_cell(trade.price),
            formatting::cash_cell(trade.commission),
            formatting::cash_cell(trade.price * trade.quantity),
        ]));
    }

    formatting::print_statement(
        "Sell simulation results",
        vec!["Instrument", "Quantity", "Price", "Commission", "Revenue"],
        table,
    );

    Err!("Not implemented yet")
}
