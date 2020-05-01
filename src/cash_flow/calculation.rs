use crate::broker_statement::{BrokerStatement, trades::StockBuy};
use crate::currency::{Cash, CashAssets, MultiCurrencyCashAccount};
use crate::types::Date;

// FIXME(konishchev): It's only a prototype
pub fn calculate(statement: &BrokerStatement) {
    let mut cash_flows = Vec::new();
    let mut cash_assets = MultiCurrencyCashAccount::new();

    cash_flows.extend(statement.cash_flows.iter().map(new_from_cash_flow));

    for trade in &statement.stock_buys {
        let (trade_cash_flow, commission_cash_flow) = new_from_stock_buy(trade);

        cash_flows.push(trade_cash_flow);
        if let Some(cash_flow) = commission_cash_flow {
            cash_flows.push(cash_flow);
        }
    }

    cash_flows.sort_by_key(|cash_flow| cash_flow.date);
    for cash_flow in &cash_flows {
        cash_assets.deposit(cash_flow.amount);
        println!("{}: {} - {}", cash_flow.date, cash_flow.description, cash_flow.amount);
    }

    for assets in cash_assets.iter() {
        println!("{}", assets);
    }

    for assets in statement.cash_assets.iter() {
        println!("{}", assets);
    }
}

struct CashFlow {
    pub date: Date,
    pub amount: Cash,
    pub description: String,
}

impl CashFlow {
    fn new(date: Date, amount: Cash, description: String) -> CashFlow {
        return CashFlow {date, amount, description}
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
    let trade_cash_flow = CashFlow::new(trade.conclusion_date, -volume, description);

    let commission_cash_flow = if !trade.commission.is_zero() {
        let description = format!("Комиссия за покупку {} {}", trade.quantity, trade.symbol);
        // FIXME(konishchev): Rounding
        Some(CashFlow::new(trade.conclusion_date, -trade.commission, description))
    } else {
        None
    };

    (trade_cash_flow, commission_cash_flow)
}