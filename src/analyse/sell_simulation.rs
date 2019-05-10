use crate::broker_statement::BrokerStatement;
use crate::broker_statement::trades::StockSell;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting::table::{Table, Row, Cell, Alignment, print_table};
use crate::localities::Country;
use crate::quotes::Quotes;

pub fn simulate_sell(
    portfolio: &PortfolioConfig, mut statement: BrokerStatement, converter: &CurrencyConverter,
    mut quotes: Quotes, positions: &[(String, Option<u32>)],
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

    print_results(stock_sells, &portfolio.get_tax_country(), converter)
}

fn print_results(stock_sells: Vec<StockSell>, country: &Country, converter: &CurrencyConverter) -> EmptyResult {
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
            fifo_table.add_row(Row::new(&[
                Cell::new(if buy_trade_id == 0 {
                    &trade.symbol
                } else {
                    ""
                }),
                Cell::new_align(&buy_trade.quantity.to_string(), Alignment::RIGHT),
                Cell::new_cash(buy_trade.price),
            ]));
        }

        let total_purchase_cost = Cash::new(
            trade.price.currency,
            total_purchase_cost.total_assets(trade.price.currency, converter)?);

        let average_buy_price = (total_purchase_cost / trade.quantity).round();

        let mut row = vec![
            Cell::new(&trade.symbol),
            Cell::new_align(&trade.quantity.to_string(), Alignment::RIGHT),
            Cell::new_cash(average_buy_price),
            Cell::new_cash(trade.price),
            Cell::new_cash(trade.commission),
            Cell::new_cash(details.revenue),
            Cell::new_cash(details.profit),
            Cell::new_cash(details.tax_to_pay),
            Cell::new_ratio(details.real_profit_ratio),
        ];

        if !same_currency {
            row.extend_from_slice(&[
                Cell::new_ratio(details.real_tax_ratio),
                Cell::new_ratio(details.real_local_profit_ratio),
            ]);
        }

        table.add_row(Row::new(&row));
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
        totals.push(Cell::new_multi_currency_cash(total));
    }
    totals.push(Cell::new_cash(tax_to_pay));
    while totals.len() < columns.len() {
        totals.push(Cell::new_align("", Alignment::RIGHT));
    }
    table.add_row(Row::new(&totals));

    print_table("Sell simulation results", &columns, table);
    print_table("FIFO details", &["Symbol", "Quantity", "Price"], fifo_table);

    Ok(())
}
