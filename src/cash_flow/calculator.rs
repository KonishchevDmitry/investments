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
    let starting_assets_date = start_date - Duration::days(1);
    let ending_assets_date = end_date - Duration::days(1);
    let comparator = CashAssetsComparator::new(
        &statement.historical_cash_assets, vec![starting_assets_date, ending_assets_date]);

    Calculator {
        statement, comparator,
        start_date, starting_assets_date, ending_assets_date,
        starting_assets: None, ending_assets: None,
        assets: MultiCurrencyCashAccount::new(),
    }.process()
}

struct Calculator<'a> {
    statement: &'a BrokerStatement,
    comparator: CashAssetsComparator<'a>,

    start_date: Date,
    starting_assets_date: Date,
    ending_assets_date: Date,

    starting_assets: Option<MultiCurrencyCashAccount>,
    ending_assets: Option<MultiCurrencyCashAccount>,
    assets: MultiCurrencyCashAccount,
}

impl<'a> Calculator<'a> {
    fn process(&mut self) -> (BTreeMap<&'static str, CashFlowSummary>, Vec<CashFlow>) {
        let cash_flows = CashFlowMapper::map(self.statement);

        // FIXME(konishchev): Rewrite all below
        for cash_flow in &cash_flows {
            self.process_date(cash_flow.date);
            self.assets.deposit(cash_flow.amount);
        }

        self.process_date(self.statement.period.1);

        // assert!(comparator.compare(statement.period.1, &assets));

        let starting_assets = self.starting_assets.clone().unwrap();
        let ending_assets = self.assets.clone();

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

    fn process_date(&mut self, date: Date) {
        self.comparator.compare(date, &self.assets);

        if self.starting_assets.is_none() && self.starting_assets_date < date {
            let historical = &self.statement.historical_cash_assets;
            self.starting_assets.replace(match historical.get(&self.starting_assets_date) {
                Some(actual) => {
                    self.assets = actual.clone();
                    actual.clone()
                },
                None => {
                    if self.statement.period.0 <= self.starting_assets_date {
                        warn!("Using calculated assets value for {}.", format_date(self.start_date));
                    }
                    self.assets.clone()
                },
            });
        }
    }
}