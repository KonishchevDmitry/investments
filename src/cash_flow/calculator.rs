use nanoid::nanoid;

use crate::broker_statement::{BrokerStatement, Fee, ForexTrade, StockBuy, StockSell, Dividend,
                              IdleCashInterest};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::types::Date;

use super::comparator::CashAssetsComparator;

// FIXME(konishchev): It's only a prototype
pub fn calculate(statement: &BrokerStatement, _start_date: Date, _end_date: Date) -> Vec<CashFlow> {
    let mut cash_flows = Vec::new();
    let mut cash_assets = MultiCurrencyCashAccount::new();

    cash_flows.extend(statement.cash_flows.iter().map(new_from_cash_flow));
    cash_flows.extend(statement.fees.iter().map(new_from_fee));
    cash_flows.extend(statement.idle_cash_interest.iter().map(new_from_interest));

    for trade in &statement.forex_trades {
        let (from, to, commission) = new_from_forex_trade(trade);

        cash_flows.push(from);
        cash_flows.push(to);
        if let Some(cash_flow) = commission {
            cash_flows.push(cash_flow);
        }
    }

    for trade in &statement.stock_buys {
        let (cash_flow, commission) = new_from_stock_buy(trade);

        cash_flows.push(cash_flow);
        if let Some(cash_flow) = commission {
            cash_flows.push(cash_flow);
        }
    }

    for trade in &statement.stock_sells {
        let (cash_flow, commission) = new_from_stock_sell(trade);

        cash_flows.push(cash_flow);
        if let Some(cash_flow) = commission {
            cash_flows.push(cash_flow);
        }
    }

    for dividend in &statement.dividends {
        let (cash_flow, paid_tax) = new_from_dividend(dividend);

        cash_flows.push(cash_flow);
        if let Some(cash_flow) = paid_tax {
            cash_flows.push(cash_flow);
        }
    }

    cash_flows.sort_by_key(|cash_flow| cash_flow.date);

    let mut cash_assets_comparator = CashAssetsComparator::new(&statement.historical_cash_assets);

    for cash_flow in &cash_flows {
        cash_assets_comparator.compare(cash_flow.date, &cash_assets);
        cash_assets.deposit(cash_flow.amount);
        println!("{}: {} - {}", cash_flow.date, cash_flow.description, cash_flow.amount);
    }
    assert!(cash_assets_comparator.compare(statement.period.1, &cash_assets));

    for assets in cash_assets.iter() {
        println!("{}", assets);
    }

    println!();
    for assets in statement.cash_assets.iter() {
        println!("{}", assets);
    }

    cash_flows
}

pub struct CashFlow {
    pub id: String,
    pub date: Date,
    pub amount: Cash,
    pub description: String,
}

impl CashFlow {
    fn new(date: Date, amount: Cash, description: String) -> CashFlow {
        CashFlow {id: nanoid!(), date, amount, description}
    }
}

fn new_from_cash_flow(assets: &CashAssets) -> CashFlow {
    let description = if assets.cash.is_positive() {
        "Ввод денежных средств"
    } else {
        "Вывод денежных средств"
    };

    CashFlow::new(assets.date, assets.cash, description.to_owned())
}

fn new_from_stock_buy(trade: &StockBuy) -> (CashFlow, Option<CashFlow>) {
    // FIXME(konishchev): Rounding
    let volume = trade.price * trade.quantity;
    let description = format!("Покупка {} {}", trade.quantity, trade.symbol);
    let cash_flow = CashFlow::new(trade.conclusion_date, -volume, description);

    let commission = if !trade.commission.is_zero() {
        let description = format!("Комиссия за покупку {} {}", trade.quantity, trade.symbol);
        // FIXME(konishchev): Rounding
        Some(CashFlow::new(trade.conclusion_date, -trade.commission, description))
    } else {
        None
    };

    (cash_flow, commission)
}

fn new_from_stock_sell(trade: &StockSell) -> (CashFlow, Option<CashFlow>) {
    // FIXME(konishchev): Rounding
    let volume = trade.price * trade.quantity;
    let description = format!("Продажа {} {}", trade.quantity, trade.symbol);
    let cash_flow = CashFlow::new(trade.conclusion_date, volume, description);

    let commission = if !trade.commission.is_zero() {
        let description = format!("Комиссия за продажу {} {}", trade.quantity, trade.symbol);
        // FIXME(konishchev): Rounding
        Some(CashFlow::new(trade.conclusion_date, -trade.commission, description))
    } else {
        None
    };

    (cash_flow, commission)
}

fn new_from_dividend(dividend: &Dividend) -> (CashFlow, Option<CashFlow>) {
    // FIXME(konishchev): Rounding
    let description = format!("Дивиденд от {}", dividend.issuer);
    let cash_flow = CashFlow::new(dividend.date, dividend.amount, description);

    let paid_tax = if !dividend.paid_tax.is_zero() {
        let description = format!("Налог, удержанный с дивиденда от {}", dividend.issuer);
        // FIXME(konishchev): Rounding
        Some(CashFlow::new(dividend.date, -dividend.paid_tax, description))
    } else {
        None
    };

    (cash_flow, paid_tax)
}

fn new_from_forex_trade(trade: &ForexTrade) -> (CashFlow, CashFlow, Option<CashFlow>) {
    // FIXME(konishchev): Rewrite
    let (from, to) = if trade.volume.is_sign_positive() {
        let from = Cash::new(&trade.base, trade.quantity);
        let to = Cash::new(&trade.quote, trade.volume);
        (from, to)
    } else {
        let from = Cash::new(&trade.quote, trade.volume);
        let to = Cash::new(&trade.base, trade.quantity);
        (from, to)
    };

    let description = format!("Конвертация {} -> {}", -from, to);
    let from_cash_flow = CashFlow::new(trade.conclusion_date, from, description.clone());
    let to_cash_flow = CashFlow::new(trade.conclusion_date, to, description);

    let commission = if !trade.commission.is_zero() {
        let description = format!("Комиссия за конвертацию {} -> {}", -from, to);
        // FIXME(konishchev): Rounding
        Some(CashFlow::new(trade.conclusion_date, -trade.commission, description))
    } else {
        None
    };

    (from_cash_flow, to_cash_flow, commission)
}

fn new_from_interest(interest: &IdleCashInterest) -> CashFlow {
    CashFlow::new(interest.date, interest.amount, "Проценты на остаток по счету".to_owned())
}

fn new_from_fee(fee: &Fee) -> CashFlow {
    CashFlow::new(fee.date, -fee.amount, "Комиссия брокера".to_owned())
}