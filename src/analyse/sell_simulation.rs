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

pub fn simulate_sell(
    mut statement: BrokerStatement, converter: &CurrencyConverter, mut quotes: Quotes,
    positions: &[(String, Option<u32>)],
) -> EmptyResult {
    for (symbol, _) in positions {
        if statement.open_positions.get(symbol).is_none() {
            return Err!("The portfolio has no open {:?} positions", symbol);
        }

        quotes.batch(&symbol);
    }

    for (symbol, quantity) in positions {
        let quantity = *match quantity {
            Some(quantity) => quantity,
            None => match statement.open_positions.get(symbol) {
                Some(quantity) => quantity,
                None => return Err!("The portfolio has no open {:?} positions", symbol),
            }
        };

        statement.emulate_sell(&symbol, quantity, quotes.get(&symbol)?)?;
    }
    statement.process_trades()?;

    let stock_sells = statement.stock_sells.iter()
        .filter(|stock_sell| stock_sell.emulation)
        .cloned().collect::<Vec<_>>();
    assert_eq!(stock_sells.len(), positions.len());

    print_results(stock_sells, converter)
}

fn print_results(stock_sells: Vec<StockSell>, converter: &CurrencyConverter) -> EmptyResult {
    let country = localities::russia();

    let mut same_currency = true;
    for trade in &stock_sells {
        same_currency &=
            trade.price.currency == country.currency &&
                trade.commission.currency == country.currency;
    }

    let mut total_commissions = MultiCurrencyCashAccount::new();
    let mut total_revenue = MultiCurrencyCashAccount::new();
    let mut total_profit = MultiCurrencyCashAccount::new();
    let mut total_local_profit = Cash::new(country.currency, dec!(0));

    let mut table = Table::new();
    let mut fifo_table = Table::new();

    for trade in stock_sells {
        let details = trade.calculate(&country, &converter)?;
        let mut total_purchase_cost = MultiCurrencyCashAccount::new();

        total_commissions.deposit(trade.commission);
        total_revenue.deposit(details.revenue);
        total_profit.deposit(details.profit);
        total_local_profit.add_assign(details.local_profit).unwrap();

        for (buy_trade_id, buy_trade) in details.fifo.iter().enumerate() {
            total_purchase_cost.deposit(buy_trade.price * buy_trade.quantity);
            fifo_table.add_row(Row::new(vec![
                Cell::new(if buy_trade_id == 0 {
                    &trade.symbol
                } else {
                    ""
                }),
                Cell::new_align(&buy_trade.quantity.to_string(), Alignment::RIGHT),
                formatting::cash_cell(buy_trade.price),
            ]));
        }

        let total_purchase_cost = Cash::new(
            trade.price.currency,
            total_purchase_cost.total_assets(trade.price.currency, converter)?);

        let average_buy_price = (total_purchase_cost / trade.quantity).round();

        let mut row = vec![
            Cell::new(&trade.symbol),
            Cell::new_align(&trade.quantity.to_string(), Alignment::RIGHT),
            formatting::cash_cell(average_buy_price),
            formatting::cash_cell(trade.price),
            formatting::cash_cell(trade.commission),
            formatting::cash_cell(details.revenue),
            formatting::cash_cell(details.profit),
            formatting::cash_cell(details.tax_to_pay),
            formatting::ratio_cell(details.real_profit_ratio),
        ];

        if !same_currency {
            row.extend_from_slice(&[
                formatting::ratio_cell(details.real_tax_ratio),
                formatting::ratio_cell(details.real_local_profit_ratio),
            ]);
        }

        table.add_row(Row::new(row));
    }

    let tax_to_pay = Cash::new(country.currency, country.tax_to_pay(total_local_profit.amount, None));

    let mut columns = vec![
        "Symbol", "Quantity", "Buy price", "Sell Price", "Commission", "Revenue",
        "Profit", "Tax to pay", "Real profit %",
    ];

    if !same_currency {
        columns.extend(&["Real tax %", "Real local profit %"]);
    }

    let mut totals = Vec::new();
    for _ in 0..4 {
        totals.push(Cell::new(""));
    }
    for total in vec![total_commissions, total_revenue, total_profit] {
        totals.push(formatting::multi_currency_cash_cell(total));
    }
    totals.push(formatting::cash_cell(tax_to_pay));
    while totals.len() < columns.len() {
        totals.push(Cell::new_align("", Alignment::RIGHT));
    }
    table.add_row(Row::new(totals));

    formatting::print_statement("Sell simulation results", &columns, table);
    formatting::print_statement("FIFO details", &["Symbol", "Quantity", "Price"], fifo_table);

    Ok(())
}
