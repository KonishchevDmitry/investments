use std::collections::BTreeMap;

use chrono::Duration;
use log::warn;

use crate::broker_statement::BrokerStatement;
use crate::currency::MultiCurrencyCashAccount;
use crate::formatting::format_date;
use crate::types::{Date, Decimal};

use super::mapper::{CashFlow, map_broker_statement_to_cash_flow};
use super::comparator::CashAssetsComparator;

pub struct CashFlowSummary {
    pub starting: Decimal,
    pub deposits: Decimal,
    pub withdrawals: Decimal,
    pub ending: Decimal,
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
        start_date, starting_assets_date,
        end_date, ending_assets_date,

        starting_assets: None,
        deposits: MultiCurrencyCashAccount::new(),
        withdrawals: MultiCurrencyCashAccount::new(),
        ending_assets: None,

        assets: MultiCurrencyCashAccount::new(),
    }.process()
}

struct Calculator<'a> {
    statement: &'a BrokerStatement,
    comparator: CashAssetsComparator<'a>,

    start_date: Date,
    starting_assets_date: Date,
    end_date: Date,
    ending_assets_date: Date,

    starting_assets: Option<MultiCurrencyCashAccount>,
    deposits: MultiCurrencyCashAccount,
    withdrawals: MultiCurrencyCashAccount,
    ending_assets: Option<MultiCurrencyCashAccount>,

    assets: MultiCurrencyCashAccount,
}

impl<'a> Calculator<'a> {
    fn process(mut self) -> (BTreeMap<&'static str, CashFlowSummary>, Vec<CashFlow>) {
        let mut cash_flows = map_broker_statement_to_cash_flow(self.statement);
        let mut begin_index = None;
        let mut end_index = None;

        for (index, cash_flow) in cash_flows.iter().enumerate() {
            if cash_flow.date < self.start_date {
                begin_index.replace(index);
            } else if end_index.is_none() && self.end_date <= cash_flow.date {
                end_index.replace(index);
            }

            self.process_date(cash_flow.date);
            self.assets.deposit(cash_flow.amount);

            if self.start_date <= cash_flow.date && cash_flow.date < self.end_date {
                if cash_flow.amount.is_negative() {
                    self.withdrawals.deposit(-cash_flow.amount);
                } else {
                    self.deposits.deposit(cash_flow.amount);
                }
            }
        }

        if let Some(index) = end_index {
            cash_flows.drain(index..);
        }

        if let Some(index) = begin_index {
            cash_flows.drain(..=index);
        }

        self.process_date(self.statement.period.1);
        assert!(self.comparator.consumed());

        let mut summaries = BTreeMap::new();
        let starting_assets = self.starting_assets.unwrap();
        let ending_assets = self.ending_assets.unwrap();

        for ending_assets in ending_assets.iter() {
            let currency = ending_assets.currency;
            let get_assets = |assets: &MultiCurrencyCashAccount| {
                assets.get(currency)
                    .map(|assets| assets.amount)
                    .unwrap_or_else(|| dec!(0))
            };

            let starting = get_assets(&starting_assets);
            let deposits = get_assets(&self.deposits);
            let withdrawals = get_assets(&self.withdrawals);
            let ending = ending_assets.amount;

            assert_eq!(ending, starting + deposits - withdrawals);
            summaries.insert(currency, CashFlowSummary {starting, deposits, withdrawals, ending});
        }

        (summaries, cash_flows)
    }

    fn process_date(&mut self, date: Date) {
        self.comparator.compare(date, &self.assets);

        if self.starting_assets.is_none() && self.starting_assets_date < date {
            self.starting_assets.replace(match self.statement.historical_cash_assets.get(&self.starting_assets_date) {
                Some(actual) => {
                    self.assets = actual.clone();
                    actual.clone()
                },
                None => {
                    if self.statement.period.0 <= self.starting_assets_date {
                        warn!(concat!(
                            "There is no information about starting cash assets for {} in the broker statement.",
                            "Using calculated value."
                        ), format_date(self.start_date));
                    }
                    self.assets.clone()
                },
            });
        }

        if self.ending_assets.is_none() && self.ending_assets_date < date {
            self.ending_assets.replace(self.assets.clone());

            if self.statement.historical_cash_assets.get(&self.ending_assets_date).is_none() {
                warn!(
                    "There is no information about ending cash assets for {} in the broker statement.",
                    format_date(self.ending_assets_date)
                );
            }
        }
    }
}