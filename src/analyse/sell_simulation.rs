use prettytable::{Table, Row, Cell};
use prettytable::format::Alignment;

use crate::broker_statement::BrokerStatement;
use crate::broker_statement::trades::StockSell;
use crate::core::EmptyResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities;
use crate::quotes::Quotes;
use crate::util;

pub fn simulate_sell(
    mut statement: BrokerStatement, converter: &CurrencyConverter, mut quotes: Quotes,
    positions: &[(String, u32)],
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

    let stock_sells = statement.stock_sells.iter()
        .filter(|stock_sell| stock_sell.emulation)
        .cloned().collect::<Vec<_>>();
    assert_eq!(stock_sells.len(), positions.len());

    print_results(stock_sells, converter)
}

// FIXME: Complete the prototype
fn print_results(stock_sells: Vec<StockSell>, converter: &CurrencyConverter) -> EmptyResult {
    let country = localities::russia();

    let mut total_commissions = MultiCurrencyCashAccount::new();
    let mut total_revenue = MultiCurrencyCashAccount::new();
    let mut total_profit = MultiCurrencyCashAccount::new();
    let mut total_tax_to_pay = MultiCurrencyCashAccount::new();

    let mut table = Table::new();
    let mut fifo_details = Vec::new();

    for trade in stock_sells {
        let details = trade.calculate(&country, &converter)?;

        total_commissions.deposit(trade.commission);
        total_revenue.deposit(details.revenue);
        total_profit.deposit(details.profit);
        total_tax_to_pay.deposit(details.tax_to_pay);

        assert_eq!(details.profit.currency, details.cost.currency);
        let profit_percent = util::round_to(
            details.profit.amount / details.cost.amount * dec!(100), 1);

        let tax_amount = converter.convert_to(
            util::today(), details.tax_to_pay, details.profit.currency)?;
        let real_tax_percent = util::round_to(
            tax_amount / details.profit.amount * dec!(100), 1);

        let mut details_table = Table::new();

        for source in &details.fifo {
            details_table.add_row(Row::new(vec![
                Cell::new_align(&source.quantity.to_string(), Alignment::RIGHT),
                formatting::cash_cell(source.price),
            ]));
        }

        table.add_row(Row::new(vec![
            Cell::new(&trade.symbol),
            Cell::new_align(&trade.quantity.to_string(), Alignment::RIGHT),
            formatting::cash_cell(trade.price),
            formatting::cash_cell(trade.commission),
            formatting::cash_cell(details.revenue),
            formatting::cash_cell(details.profit),
            formatting::cash_cell(details.tax_to_pay),
            Cell::new_align(&format!("{}%", profit_percent), Alignment::RIGHT),
            Cell::new_align(&format!("{}%", real_tax_percent), Alignment::RIGHT),
        ]));

        fifo_details.push((trade.symbol, details_table));
    }

    let mut totals = Vec::new();
    for _ in 0..3 {
        totals.push(Cell::new(""));
    }
    for total in &[total_commissions, total_revenue, total_profit, total_tax_to_pay] {
        let mut assets_iter = total.iter();

        let cell = if assets_iter.len() == 1 {
            let (currency, &amount) = assets_iter.next().unwrap();
            formatting::cash_cell(Cash::new(currency, amount))
        } else {
            Cell::new("")
        };

        totals.push(cell);
    }
    for _ in 0..2 {
        totals.push(Cell::new_align("", Alignment::RIGHT));
    }
    table.add_row(Row::new(totals));

    formatting::print_statement(
        "Sell simulation results",
        &[
            "Instrument", "Quantity", "Price", "Commission", "Revenue", "Profit", "Tax to pay",
            "Profit %", "Real tax %",
        ],
        table,
    );

    for (symbol, details_table) in fifo_details {
        formatting::print_statement(
            &format!("FIFO details for {}", symbol),
            &["Quantity", "Price"],
            details_table,
        );
    }

    Ok(())
}
