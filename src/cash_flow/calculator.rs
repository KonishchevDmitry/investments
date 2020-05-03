use std::collections::BTreeMap;

use chrono::Duration;
use log::warn;

use crate::broker_statement::{BrokerStatement};
use crate::currency::{MultiCurrencyCashAccount, Cash};
use crate::formatting::format_date;
use crate::types::{Date, Decimal};

use super::mapper::{CashFlowMapper, CashFlow};
use super::comparator::CashAssetsComparator;

pub struct CashFlowSummary {
    pub start: Decimal,
    pub deposits: Decimal,
    pub withdrawals: Decimal,
    pub end: Decimal,
}

pub fn calculate(statement: &BrokerStatement, start_date: Date, end_date: Date) -> (
    BTreeMap<&'static str, CashFlowSummary>, Vec<CashFlow>
) {
    let cash_flows = CashFlowMapper::map(statement);

    let starting_assets_date = start_date - Duration::days(1);
    let ending_assets_date = end_date - Duration::days(1);
    let mut comparator = CashAssetsComparator::new(
        &statement.historical_cash_assets, vec![starting_assets_date, ending_assets_date]);

    let mut starting_assets = None;
    let mut assets = MultiCurrencyCashAccount::new();

    // FIXME(konishchev): Rewrite all below
    for cash_flow in &cash_flows {
        comparator.compare(cash_flow.date, &assets);

        if starting_assets.is_none() && starting_assets_date < cash_flow.date {
            starting_assets.replace(match statement.historical_cash_assets.get(&starting_assets_date) {
                Some(actual) => {
                    assets = actual.clone();
                    actual.clone()
                },
                None => {
                    if statement.period.0 <= starting_assets_date {
                        warn!("Using calculated assets value for {}.", format_date(start_date));
                    }
                    assets.clone()
                },
            });
        }

        assets.deposit(cash_flow.amount);
    }
    assert!(comparator.compare(statement.period.1, &assets));

    if false {
        for assets in assets.iter() {
            println!("{}", assets);
        }

        println!();
        for assets in statement.cash_assets.iter() {
            println!("{}", assets);
        }
    }

    let starting_assets = starting_assets.unwrap();
    let ending_assets = assets.clone();

    let mut summaries = BTreeMap::new();
    for assets in ending_assets.iter() {
        summaries.insert(assets.currency, CashFlowSummary {
            start: starting_assets.get(assets.currency).unwrap_or_else(|| Cash::new(assets.currency, dec!(0))).amount,
            deposits: dec!(0),
            withdrawals: dec!(0),
            end: assets.amount,
        });
    }

    (summaries, cash_flows)
}