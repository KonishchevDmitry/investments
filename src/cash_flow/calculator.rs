use std::collections::BTreeMap;

use log::warn;

use crate::broker_statement::BrokerStatement;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::formatting::format_date;
use crate::time::{Date, Period};
use crate::types::Decimal;

use super::mapper::{CashFlow, map_broker_statement_to_cash_flow};
use super::comparator::CashAssetsComparator;

pub struct CashFlowSummary {
    pub starting: Decimal,
    pub deposits: Decimal,
    pub withdrawals: Decimal,
    pub ending: Decimal,
}

pub fn calculate(statement: &BrokerStatement, period: Period) -> (
    BTreeMap<&'static str, CashFlowSummary>, Vec<CashFlow>
) {
    let historical_cash_assets = statement.historical_assets.iter().map(|(&date, assets)| {
        (date, assets.cash.clone())
    }).collect();

    let starting_assets_date = period.prev_date();
    let ending_assets_date = period.last_date();

    let comparator = CashAssetsComparator::new(
        &historical_cash_assets, vec![starting_assets_date, ending_assets_date]);

    Calculator {
        statement, comparator,
        period, starting_assets_date, ending_assets_date,

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

    period: Period,
    starting_assets_date: Date,
    ending_assets_date: Date,

    starting_assets: Option<MultiCurrencyCashAccount>,
    deposits: MultiCurrencyCashAccount,
    withdrawals: MultiCurrencyCashAccount,
    ending_assets: Option<MultiCurrencyCashAccount>,

    assets: MultiCurrencyCashAccount,
}

impl Calculator<'_> {
    fn process(mut self) -> (BTreeMap<&'static str, CashFlowSummary>, Vec<CashFlow>) {
        let mut cash_flows = map_broker_statement_to_cash_flow(self.statement);
        let mut begin_index = None;
        let mut end_index = None;

        for (index, cash_flow) in cash_flows.iter().enumerate() {
            if cash_flow.time.date < self.period.first_date() {
                begin_index.replace(index);
            } else if end_index.is_none() && self.period.last_date() < cash_flow.time.date {
                end_index.replace(index);
            }

            self.process_date(cash_flow.time.date);

            self.process_cash_flow(cash_flow.time.date, cash_flow.amount);
            if let Some(amount) = cash_flow.sibling_amount {
                self.process_cash_flow(cash_flow.time.date, amount);
            }
        }

        if let Some(index) = end_index {
            cash_flows.drain(index..);
        }

        if let Some(index) = begin_index {
            cash_flows.drain(..=index);
        }

        self.process_date(self.statement.period.next_date());
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
            self.starting_assets.replace(self.assets.clone());

            if !self.statement.historical_assets.contains_key(&self.starting_assets_date) {
                if self.statement.period.first_date() <= self.starting_assets_date {
                    warn!(
                        "There is no information about starting cash assets for {} in the broker statement.",
                        format_date(self.period.first_date())
                    );
                }
            }
        }

        if self.ending_assets.is_none() && self.ending_assets_date < date {
            self.ending_assets.replace(self.assets.clone());

            if !self.statement.historical_assets.contains_key(&self.ending_assets_date) {
                warn!(
                    "There is no information about ending cash assets for {} in the broker statement.",
                    format_date(self.ending_assets_date)
                );
            }
        }
    }

    fn process_cash_flow(&mut self, date: Date, amount: Cash) {
        self.assets.deposit(amount);

        if self.period.contains(date) {
            if amount.is_negative() {
                self.withdrawals.deposit(-amount);
            } else {
                self.deposits.deposit(amount);
            }
        }
    }
}