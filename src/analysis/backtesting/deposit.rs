use std::collections::VecDeque;

use chrono::Days;
use log::{trace, debug};

use crate::analysis::deposit::Transaction;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::currency::converter::CurrencyConverterRc;
use crate::formatting;
use crate::time::{Date, Period};
use crate::types::Decimal;

use super::BenchmarkPerformanceType;
use super::benchmark::{Benchmark, BacktestingResult};

pub struct DepositBenchmark {
    name: String,
    currency: &'static str,
    duration: u64,
    replenishment_period: u64,
    converter: CurrencyConverterRc,
}

impl DepositBenchmark {
    pub fn new(name: &str, currency: &'static str, duration: u64, replenishment_period: u64, converter: CurrencyConverterRc) -> Self {
        DepositBenchmark {
            name: name.to_owned(),
            currency,
            duration,
            replenishment_period,
            converter,
        }
    }
}

impl Benchmark for DepositBenchmark {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn provider(&self) -> Option<String> {
        None
    }

    fn backtest(
        &self, method: BenchmarkPerformanceType, currency: &str, cash_flows: &[CashAssets], today: Date, full: bool,
    ) -> GenericResult<Vec<BacktestingResult>> {
        Backtester {
            method, currency,
            converter: self.converter.clone(),

            benchmark: self,
            cash_flows,
            transactions: Vec::new(),
            results: Vec::new(),
            full,

            date: cash_flows.first().unwrap().date,
            today,
            deposits: VecDeque::new(),
        }.backtest()
    }
}

struct Backtester<'a> {
    method: BenchmarkPerformanceType,
    currency: &'a str,
    converter: CurrencyConverterRc,

    benchmark: &'a DepositBenchmark,
    cash_flows: &'a [CashAssets],
    transactions: Vec<Transaction>,
    results: Vec<BacktestingResult>,
    full: bool,

    date: Date,
    today: Date,
    deposits: VecDeque<Deposit>,
}

impl Backtester<'_> {
    fn backtest(mut self) -> GenericResult<Vec<BacktestingResult>> {
        for cash_flow in self.cash_flows {
            debug!("Backtesting cash flow: {}: {}...", formatting::format_date(cash_flow.date), cash_flow.cash);
            self.process_to(cash_flow.date)?;

            let assets = self.converter.convert_to(cash_flow.date, cash_flow.cash, self.currency)?;
            self.transactions.push(Transaction::new(cash_flow.date, assets));

            let assets = self.converter.convert_to(cash_flow.date, cash_flow.cash, self.benchmark.currency)?;
            self.process_cash_flow(cash_flow.date, assets)?;
        }

        self.process_to(self.today)?;
        self.close_day(true)?;

        Ok(self.results)
    }

    fn process_to(&mut self, date: Date) -> EmptyResult {
        assert!(date >= self.date);

        while self.date != date {
            self.close_day(self.full)?;
        }

        Ok(())
    }

    fn close_day(&mut self, calculate: bool) -> EmptyResult {
        let date = self.date;
        assert!(date <= self.today);

        if let Some(deposit) = self.deposits.front() {
            if date == deposit.close_date {
                let assets = self.deposits.pop_front().unwrap().close(date, false);
                if !assets.is_zero() {
                    self.process_cash_flow(date, assets)?;
                }
            } else {
                assert!(date < deposit.close_date, "open: {}, close: {}, current: {date}",
                    deposit.open_date, deposit.close_date);
            }
        }

        if calculate {
            let assets = Cash::new(
                self.benchmark.currency,
                self.deposits.iter().map(|deposit| {
                    assert!(date < deposit.close_date);
                    deposit.amount
                }).sum()
            );

            let net_value = self.converter.convert_to_cash(date, assets, self.currency)?;
            let min_days_for_performance = if self.benchmark.currency == self.currency {
                1
            } else {
                365 // FIXME(konishchev): To config
            };

            self.results.push(BacktestingResult::calculate(
                &self.benchmark.name(), date, net_value,
                self.method, &self.transactions, min_days_for_performance)?);
        }

        self.date = date.succ_opt().unwrap();
        Ok(())
    }

    fn process_cash_flow(&mut self, date: Date, amount: Decimal) -> EmptyResult {
        if amount.is_sign_negative() {
            while let Some(deposit) = self.deposits.back() {
                if deposit.amount >= amount || self.deposits.len() == 1 {
                    break;
                }

                let assets = self.deposits.pop_back().unwrap().close(date, false);
                self.deposits.back_mut().unwrap().transaction(date, assets);
            }
        }

        let deposit = match self.deposits.back_mut() {
            Some(deposit) if (
                amount.is_sign_negative() ||
                (date - deposit.open_date).num_days() < self.benchmark.replenishment_period.try_into().unwrap()
            ) => deposit,
            _ => self.open_new_deposit(date)?,
        };

        deposit.transaction(date, amount);
        Ok(())
    }

    fn open_new_deposit(&mut self, date: Date) -> GenericResult<&mut Deposit> {
        let interest = dec!(0); // FIXME(konishchev): Implement
        self.deposits.push_back(Deposit::new(date, self.benchmark.duration, interest)?);
        Ok(self.deposits.back_mut().unwrap())
    }
}

struct Deposit {
    open_date: Date,
    close_date: Date,
    #[allow(dead_code)] // FIXME(konishchev): Drop it
    interest: Decimal,

    amount: Decimal,
    transactions: Vec<Transaction>,
}

impl Deposit {
    fn new(date: Date, duration: u64, interest: Decimal) -> GenericResult<Deposit> {
        let close_date = date.checked_add_days(Days::new(duration)).ok_or_else(|| format!(
            "Invalid date: {}", formatting::format_date(date)))?;

        debug!("{}: Opening new {}% deposit: {}.",
            formatting::format_date(date), interest, Period::new(date, close_date)?);

        Ok(Deposit {
            open_date: date,
            close_date,
            interest,

            amount: dec!(0),
            transactions: Vec::new(),
        })
    }

    fn transaction(&mut self, date: Date, amount: Decimal) {
        if let Some(last) = self.transactions.last() {
            assert!(last.date <= date);
        }

        trace!("{}: {}{} to {} deposit.",
            formatting::format_date(date),
            amount.is_sign_positive().then_some("+").unwrap_or_default(),
            amount, formatting::format_date(self.open_date));

        self.transactions.push(Transaction::new(date, amount));
        self.amount += amount;
    }

    fn close(self, date: Date, lose_income: bool) -> Decimal {
        if date == self.close_date {
            debug!("{}: Closing {} deposit.", formatting::format_date(date), self.open_date);
        } else {
            assert!(date < self.close_date);

            debug!("{}: Early closing {} deposit{}.",
                formatting::format_date(date),
                Period::new(self.open_date, self.close_date).unwrap(),
                lose_income.then_some(" with income losing").unwrap_or_default());

            if lose_income {
                return self.amount;
            }
        }

        // FIXME(konishchev): Don't interest negative amount
        self.amount // FIXME(konishchev): Implement calculation
    }
}