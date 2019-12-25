use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
use crate::broker_statement::trades::StockSell;
use crate::commissions::CommissionCalc;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting::table::Cell;
use crate::localities::Country;
use crate::quotes::Quotes;

pub fn simulate_sell(
    portfolio: &PortfolioConfig, mut statement: BrokerStatement, converter: &CurrencyConverter,
    mut quotes: Quotes, positions: &[(String, Option<u32>)],
) -> EmptyResult {
    let mut commission_calc = CommissionCalc::new(statement.broker.commission_spec.clone());

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

        statement.emulate_sell(&mut commission_calc, &symbol, quantity, quotes.get(&symbol)?)?;
    }
    statement.process_trades()?;

    let stock_sells = statement.stock_sells.iter()
        .filter(|stock_sell| stock_sell.emulation)
        .cloned().collect::<Vec<_>>();
    assert_eq!(stock_sells.len(), positions.len());

    print_results(stock_sells, &portfolio.get_tax_country(), converter)
}

#[derive(StaticTable)]
#[table(name="TradesTable")]
struct TradeRow {
    #[column(name="Symbol")]
    symbol: String,
    #[column(name="Quantity")]
    quantity: u32,
    #[column(name="Buy price")]
    buy_price: Cash,
    #[column(name="Sell price")]
    sell_price: Cash,
    #[column(name="Commission")]
    commission: Cash,
    #[column(name="Revenue")]
    revenue: Cash,
    #[column(name="Profit")]
    profit: Cash,
    #[column(name="Tax to pay")]
    tax_to_pay: Cash,
    #[column(name="Real profit %")]
    real_profit: Cell,
    #[column(name="Real tax %")]
    real_tax: Cell,
    #[column(name="Real local profit %")]
    real_local_profit: Cell,
}

#[derive(StaticTable)]
#[table(name="FifoTable")]
struct FifoRow {
    #[column(name="Symbol")]
    symbol: Option<String>,
    #[column(name="Quantity")]
    quantity: u32,
    #[column(name="Price")]
    price: Cash,
}

fn print_results(stock_sells: Vec<StockSell>, country: &Country, converter: &CurrencyConverter) -> EmptyResult {
    let mut same_currency = true;
    for trade in &stock_sells {
        same_currency &=
            trade.price.currency == country.currency &&
                trade.commission.currency == country.currency;
    }

    let mut total_commission = MultiCurrencyCashAccount::new();
    let mut total_revenue = MultiCurrencyCashAccount::new();
    let mut total_profit = MultiCurrencyCashAccount::new();
    let mut total_local_profit = Cash::new(country.currency, dec!(0));

    let mut trades_table = TradesTable::new();
    if same_currency {
        trades_table.hide_real_tax();
        trades_table.hide_real_local_profit();
    }

    let mut fifo_table = FifoTable::new();

    for trade in stock_sells {
        let details = trade.calculate(&country, &converter)?;
        let mut total_purchase_cost = MultiCurrencyCashAccount::new();

        total_commission.deposit(trade.commission);
        total_revenue.deposit(details.revenue);
        total_profit.deposit(details.profit);
        total_local_profit.add_assign(details.local_profit).unwrap();

        for (index, buy_trade) in details.fifo.iter().enumerate() {
            total_purchase_cost.deposit(buy_trade.price * buy_trade.quantity);
            fifo_table.add_row(FifoRow {
                symbol: if index == 0 {
                   Some(trade.symbol.clone())
                } else {
                   None
                },
                quantity: buy_trade.quantity,
                price: buy_trade.price,
            });
        }

        let total_purchase_cost = Cash::new(
            trade.price.currency,
            total_purchase_cost.total_assets(trade.price.currency, converter)?);

        let average_buy_price = (total_purchase_cost / trade.quantity).round();

        trades_table.add_row(TradeRow {
            symbol: trade.symbol,
            quantity: trade.quantity,
            buy_price: average_buy_price,
            sell_price: trade.price,
            commission: trade.commission,
            revenue: details.revenue,
            profit: details.profit,
            tax_to_pay: details.tax_to_pay,
            real_profit: Cell::new_ratio(details.real_profit_ratio),
            real_tax: Cell::new_ratio(details.real_tax_ratio),
            real_local_profit: Cell::new_ratio(details.real_local_profit_ratio),
        });
    }

    let tax_to_pay = Cash::new(country.currency, country.tax_to_pay(total_local_profit.amount, None));

    let mut totals = trades_table.add_empty_row();
    totals.set_commission(total_commission);
    totals.set_revenue(total_revenue);
    totals.set_profit(total_profit);
    totals.set_tax_to_pay(tax_to_pay);

    trades_table.print("Sell simulation results");
    fifo_table.print("FIFO details");

    Ok(())
}
