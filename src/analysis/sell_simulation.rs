use static_table_derive::StaticTable;

use crate::broker_statement::{BrokerStatement, StockSell, StockSellType, StockSourceDetails};
use crate::commissions::CommissionCalc;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::{CurrencyConverter, CurrencyConverterRc};
use crate::formatting::table::Cell;
use crate::localities::Country;
use crate::quotes::Quotes;
use crate::taxes::IncomeType;
use crate::trades::convert_price;
use crate::types::Decimal;
use crate::util;

pub fn simulate_sell(
    country: &Country, portfolio: &PortfolioConfig, mut statement: BrokerStatement,
    converter: CurrencyConverterRc, quotes: &Quotes,
    mut positions: Vec<(String, Option<Decimal>)>, base_currency: Option<&str>,
) -> EmptyResult {
    if positions.is_empty() {
        positions = statement.open_positions.keys()
            .map(|symbol| (symbol.to_owned(), None))
            .collect();
        positions.sort();
    } else {
        for (symbol, _) in &positions {
            if statement.open_positions.get(symbol).is_none() {
                return Err!("The portfolio has no open {:?} positions", symbol);
            }
        }
    }

    let net_value = statement.net_value(&converter, quotes, portfolio.currency()?)?;
    let mut commission_calc = CommissionCalc::new(
        converter.clone(), statement.broker.commission_spec.clone(), net_value)?;

    for (symbol, quantity) in &positions {
        let quantity = *match quantity {
            Some(quantity) => quantity,
            None => statement.open_positions.get(symbol).ok_or_else(|| format!(
                "The portfolio has no open {:?} positions", symbol))?,
        };

        let mut price = quotes.get(&symbol)?;
        if let Some(base_currency) = base_currency {
            price = convert_price(price, quantity, base_currency, &converter)?;
        }

        statement.emulate_sell(&symbol, quantity, price, &mut commission_calc)?;
    }

    statement.process_trades(None)?;
    let additional_commissions = statement.emulate_commissions(commission_calc)?;

    let stock_sells = statement.stock_sells.iter()
        .filter(|stock_sell| stock_sell.emulation)
        .cloned().collect::<Vec<_>>();
    assert_eq!(stock_sells.len(), positions.len());

    print_results(country, portfolio, stock_sells, additional_commissions, &converter)
}

#[derive(StaticTable)]
#[table(name="TradesTable")]
struct TradeRow {
    #[column(name="Symbol")]
    symbol: String,
    #[column(name="Quantity")]
    quantity: Decimal,
    #[column(name="Buy price")]
    buy_price: Cash,
    #[column(name="Sell price")]
    sell_price: Cash,
    #[column(name="Commission")]
    commission: Cash,
    #[column(name="Revenue")]
    revenue: Cash,
    #[column(name="Local revenue")]
    local_revenue: Cash,
    #[column(name="Profit")]
    profit: Cash,
    #[column(name="Local profit")]
    local_profit: Cash,
    #[column(name="Taxable profit")]
    taxable_local_profit: Cash,
    #[column(name="Tax to pay")]
    tax_to_pay: Cash,
    #[column(name="Tax deduction")]
    tax_deduction: Cash,
    #[column(name="Real profit %")]
    real_profit: Option<Cell>,
    #[column(name="Real tax %")]
    real_tax: Option<Cell>,
    #[column(name="Real local profit %")]
    real_local_profit: Option<Cell>,
}

#[derive(StaticTable)]
#[table(name="FifoTable")]
struct FifoRow {
    #[column(name="Symbol")]
    symbol: Option<String>,
    #[column(name="Quantity")]
    quantity: Decimal,
    #[column(name="Price")]
    price: Cash,
    #[column(name="Tax free", align="center")]
    tax_free: Option<String>,
}

fn print_results(
    country: &Country, portfolio: &PortfolioConfig,
    stock_sells: Vec<StockSell>, additional_commissions: MultiCurrencyCashAccount,
    converter: &CurrencyConverter,
) -> EmptyResult {
    let mut same_currency = true;

    let conclusion_date = util::today_trade_conclusion_date();
    let execution_date = util::today_trade_execution_date();

    let mut total_revenue = MultiCurrencyCashAccount::new();
    let mut total_local_revenue = Cash::new(country.currency, dec!(0));

    let mut total_profit = MultiCurrencyCashAccount::new();
    let mut total_local_profit = Cash::new(country.currency, dec!(0));
    let mut total_taxable_local_profit = Cash::new(country.currency, dec!(0));

    let mut total_commission = MultiCurrencyCashAccount::new();
    let mut total_tax_deduction = Cash::new(country.currency, dec!(0));

    for commission in additional_commissions.iter() {
        total_commission.deposit(commission.round());

        let local_commission = converter.convert_to_cash_rounding(
            conclusion_date, commission, country.currency)?;

        total_profit.withdraw(commission);
        total_local_profit.sub_assign(local_commission).unwrap();
        total_taxable_local_profit.sub_assign(local_commission).unwrap();
    }

    let mut trades_table = TradesTable::new();
    let mut fifo_table = FifoTable::new();
    let mut tax_exemptions = false;

    for trade in stock_sells {
        let (sell_price, commission) = match trade.type_ {
            StockSellType::Trade {price, commission, ..} => {
                same_currency &=
                    price.currency == country.currency &&
                    commission.currency == country.currency;

                (price, commission.round())
            },
            _ => unreachable!(),
        };
        total_commission.deposit(commission);

        let (tax_year, _) = portfolio.tax_payment_day().get(trade.execution_date, true);
        let details = trade.calculate(&country, tax_year, &portfolio.tax_exemptions, &converter)?;
        tax_exemptions |= details.tax_exemption_applied();

        total_revenue.deposit(details.revenue);
        total_local_revenue.add_assign(details.local_revenue).unwrap();

        total_profit.deposit(details.profit);
        total_local_profit.add_assign(details.local_profit).unwrap();
        total_taxable_local_profit.add_assign(details.taxable_local_profit).unwrap();

        total_tax_deduction.add_assign(details.tax_deduction).unwrap();

        let price_precision = std::cmp::max(2, util::decimal_precision(sell_price.amount));
        let mut purchase_cost = Cash::new(sell_price.currency, dec!(0));

        // FIXME(konishchev): HERE
        for (index, buy_trade) in details.fifo.iter().enumerate() {
            // FIXME(konishchev): Change currency?
            let price = match buy_trade.source {
                StockSourceDetails::Trade {price, ..} => {
                    buy_trade.price(price.currency, converter)?
                },
                StockSourceDetails::CorporateAction => {
                    buy_trade.price(sell_price.currency, converter)?
                }
            };
            purchase_cost.add_assign(buy_trade.cost(purchase_cost.currency, converter)?).unwrap();

            fifo_table.add_row(FifoRow {
                symbol: if index == 0 {
                   Some(trade.symbol.clone())
                } else {
                   None
                },
                quantity: (buy_trade.quantity * buy_trade.multiplier).normalize(),
                price: (price / buy_trade.multiplier).normalize(),
                tax_free: if buy_trade.tax_exemption_applied {
                    Some("✔".to_owned())
                } else {
                    None
                },
            });
        }

        trades_table.add_row(TradeRow {
            symbol: trade.symbol,
            quantity: trade.quantity,
            buy_price: (purchase_cost / trade.quantity).round_to(price_precision).normalize(),
            sell_price,
            commission,

            revenue: details.revenue,
            local_revenue: details.local_revenue,

            profit: details.profit,
            local_profit: details.local_profit,
            taxable_local_profit: details.taxable_local_profit,

            tax_to_pay: details.tax_to_pay,
            tax_deduction: details.tax_deduction,

            real_profit: details.real_profit_ratio.map(Cell::new_ratio),
            real_tax: details.real_tax_ratio.map(Cell::new_ratio),
            real_local_profit: details.real_local_profit_ratio.map(Cell::new_ratio),
        });
    }

    if same_currency {
        trades_table.hide_local_revenue();
        trades_table.hide_local_profit();
        trades_table.hide_real_tax();
        trades_table.hide_real_local_profit();
    }
    if !tax_exemptions {
        trades_table.hide_taxable_local_profit();
        trades_table.hide_tax_deduction();
        fifo_table.hide_tax_free();
    }

    let (tax_year, _) = portfolio.tax_payment_day().get(execution_date, true);
    let tax_to_pay = Cash::new(country.currency, country.tax_to_pay(
        IncomeType::Trading, tax_year, total_taxable_local_profit.amount, None));

    let mut totals = trades_table.add_empty_row();
    totals.set_commission(total_commission);
    totals.set_revenue(total_revenue);
    totals.set_local_revenue(total_local_revenue);
    totals.set_profit(total_profit);
    totals.set_local_profit(total_local_profit);
    totals.set_taxable_local_profit(total_taxable_local_profit);
    totals.set_tax_to_pay(tax_to_pay);
    totals.set_tax_deduction(total_tax_deduction);

    trades_table.print("Sell simulation results");
    fifo_table.print("FIFO details");

    Ok(())
}
