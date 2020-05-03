use std::collections::BTreeMap;

use crate::broker_statement::{BrokerStatement};
use crate::currency::MultiCurrencyCashAccount;
use crate::types::{Date, Decimal};

use super::mapper::{CashFlowMapper, CashFlow};
use super::comparator::CashAssetsComparator;

// FIXME(konishchev): Rewrite all below
pub fn calculate(statement: &BrokerStatement, _start_date: Date, _end_date: Date) -> (
    BTreeMap<&'static str, CashFlowSummary>, Vec<CashFlow>
) {
    let cash_flows = CashFlowMapper::map(statement);
    let mut cash_assets = MultiCurrencyCashAccount::new();
    let mut cash_assets_comparator = CashAssetsComparator::new(&statement.historical_cash_assets);

    for cash_flow in &cash_flows {
        cash_assets_comparator.compare(cash_flow.date, &cash_assets);
        cash_assets.deposit(cash_flow.amount);
        if false {
            println!("{}: {} - {}", cash_flow.date, cash_flow.description, cash_flow.amount);
        }
    }
    assert!(cash_assets_comparator.compare(statement.period.1, &cash_assets));

    if false {
        for assets in cash_assets.iter() {
            println!("{}", assets);
        }

        println!();
        for assets in statement.cash_assets.iter() {
            println!("{}", assets);
        }
    }

    (BTreeMap::new(), cash_flows)
}

pub struct CashFlowSummary {
    pub start: Decimal,
    pub deposits: Decimal,
    pub withdrawals: Decimal,
    pub end: Decimal,
}