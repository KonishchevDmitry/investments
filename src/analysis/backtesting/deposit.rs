use std::collections::{BTreeMap, VecDeque};

use chrono::{Datelike, Days};
use log::{trace, debug};

use crate::analysis::deposit::{DepositEmulator, Transaction};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::currency::converter::CurrencyConverterRc;
#[cfg(test)] use crate::currency::converter::CurrencyConverter;
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
    interest_rates: Option<BTreeMap<i32, Vec<Decimal>>>,
    converter: CurrencyConverterRc,
}

impl DepositBenchmark {
    pub fn new(
        name: &str, currency: &'static str, duration: u64, replenishment_period: u64,
        interest_rates: Option<BTreeMap<i32, Vec<Decimal>>>, converter: CurrencyConverterRc,
    ) -> Self {
        DepositBenchmark {
            name: name.to_owned(),
            currency,
            duration,
            replenishment_period,
            interest_rates,
            converter,
        }
    }

    fn get_interest_rate(&self, date: Date, max_age: i32) -> GenericResult<Decimal> {
        assert!(max_age < 12);

        let Some(interest_rates) = self.interest_rates.as_ref() else {
            return Ok(dec!(0));
        };


        if let Some(rates) = interest_rates.get(&date.year()) {
            let month = std::cmp::min(date.month() as i32, rates.len() as i32);

            if date.month() as i32 - month > max_age {
                return Err!("There is no deposit interest rates statistics for {}: the last known date is {}.{:02}",
                    formatting::format_date(date), date.year(), rates.len())
            }

            return Ok(rates[month as usize - 1]);
        }

        if let Some((&year, rates)) = interest_rates.range(..&date.year()).last() {
            if (date.year() * 12 + date.month() as i32) - (year * 12 + rates.len() as i32) > max_age {
                return Err!("There is no deposit interest rates statistics for {}: the last known date is {}.{:02}",
                    formatting::format_date(date), year, rates.len())
            }
            return Ok(rates[rates.len() - 1]);
        }

        Err!("There is no deposit interest rates statistics for {}: the first interest rate is at {}",
            formatting::format_date(date), *interest_rates.first_key_value().unwrap().0)
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
        &self, method: BenchmarkPerformanceType, currency: &str, cash_flows: &[CashAssets], today: Date, full: Option<Date>,
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
    full: Option<Date>,

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
            self.close_day(self.full.is_some())?;
        }

        Ok(())
    }

    fn close_day(&mut self, calculate: bool) -> EmptyResult {
        let date = self.date;
        assert!(date <= self.today);

        if let Some(deposit) = self.deposits.front() {
            if date == deposit.close_date {
                let assets = self.deposits.pop_front().unwrap().close(date);
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
            let performance_from = std::cmp::min(self.full.unwrap_or(self.today), self.today);

            self.results.push(BacktestingResult::calculate(
                &self.benchmark.full_name(), date, net_value,
                self.method, &self.transactions, self.date >= performance_from)?);
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

                let assets = self.deposits.pop_back().unwrap().close(date);
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
        let interest = self.benchmark.get_interest_rate(date, 3)?;
        self.deposits.push_back(Deposit::new(date, self.benchmark.duration, interest)?);
        Ok(self.deposits.back_mut().unwrap())
    }
}

struct Deposit {
    open_date: Date,
    close_date: Date,
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

    fn close(self, date: Date) -> Decimal {
        if date == self.close_date {
            debug!("{}: Closing {} deposit.", formatting::format_date(date), formatting::format_date(self.open_date));
        } else {
            assert!(date < self.close_date);
            debug!("{}: Early closing {} deposit.",
                formatting::format_date(date),
                Period::new(self.open_date, self.close_date).unwrap());
        }

        DepositEmulator::new(self.open_date, date, self.interest)
            .with_monthly_capitalization(false)
            .emulate(&self.transactions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_interest_rate_cash() {
        let benchmark = mock_benchmark(None);

        assert_eq!(
            benchmark.get_interest_rate(date!(2025, 1, 1), 3).unwrap(),
            dec!(0),
        );
    }

    #[test]
    fn get_interest_rate() {
        let benchmark = mock_benchmark(Some(btreemap! {
            2025 => vec![dec!(1.25), dec!(2.25), dec!(3.25)],
            2024 => vec![
                dec!(1.24), dec!(2.24), dec!(3.24), dec!(4.24), dec!(5.24), dec!(6.24),
                dec!(7.24), dec!(8.24), dec!(9.24), dec!(10.24), dec!(11.24), dec!(12.24),
            ],
            2023 => vec![
                dec!(1.23), dec!(2.23), dec!(3.23), dec!(4.23), dec!(5.23), dec!(6.23),
                dec!(7.23), dec!(8.23), dec!(9.23), dec!(10.23), dec!(11.23), dec!(12.23),
            ],
        }));

        // Exact matches

        assert_eq!(benchmark.get_interest_rate(date!(2024,  1,  1), 3).unwrap(), dec!( 1.24));
        assert_eq!(benchmark.get_interest_rate(date!(2024, 12, 12), 3).unwrap(), dec!(12.24));

        assert_eq!(benchmark.get_interest_rate(date!(2025, 1, 1), 3).unwrap(), dec!(1.25));
        assert_eq!(benchmark.get_interest_rate(date!(2025, 3, 3), 3).unwrap(), dec!(3.25));

        // Outdated matches

        assert_eq!(benchmark.get_interest_rate(date!(2025, 4, 4), 3).unwrap(), dec!(3.25));
        assert_eq!(benchmark.get_interest_rate(date!(2025, 5, 5), 3).unwrap(), dec!(3.25));
        assert_eq!(benchmark.get_interest_rate(date!(2025, 6, 6), 3).unwrap(), dec!(3.25));

        assert_eq!(
            benchmark.get_interest_rate(date!(2025, 7, 7), 3).unwrap_err().to_string(),
            "There is no deposit interest rates statistics for 07.07.2025: the last known date is 2025.03",
        );
        assert_eq!(
            benchmark.get_interest_rate(date!(2026, 1, 1), 3).unwrap_err().to_string(),
            "There is no deposit interest rates statistics for 01.01.2026: the last known date is 2025.03",
        );

        // From previous year

        assert_eq!(
            benchmark.get_interest_rate(date!(2026, 3, 3), 11).unwrap_err().to_string(),
            "There is no deposit interest rates statistics for 03.03.2026: the last known date is 2025.03",
        );

        assert_eq!(benchmark.get_interest_rate(date!(2026,  2,  2), 11).unwrap(), dec!(3.25));
        assert_eq!(benchmark.get_interest_rate(date!(2026,  1,  1), 11).unwrap(), dec!(3.25));
        assert_eq!(benchmark.get_interest_rate(date!(2025, 12, 12), 11).unwrap(), dec!(3.25));
        assert_eq!(benchmark.get_interest_rate(date!(2025,  4,  4), 11).unwrap(), dec!(3.25));
        assert_eq!(benchmark.get_interest_rate(date!(2025,  3,  3), 11).unwrap(), dec!(3.25));
        assert_eq!(benchmark.get_interest_rate(date!(2025,  2,  2), 11).unwrap(), dec!(2.25));

        // Too early

        assert_eq!(
            benchmark.get_interest_rate(date!(2022, 12, 12), 3).unwrap_err().to_string(),
            "There is no deposit interest rates statistics for 12.12.2022: the first interest rate is at 2023",
        );
    }

    fn mock_benchmark(interest_rates: Option<BTreeMap<i32, Vec<Decimal>>>) -> DepositBenchmark {
        DepositBenchmark {
            name: s!("Mock"),
            currency: "RUB",
            duration: 365,
            replenishment_period: 30,
            interest_rates,
            converter: CurrencyConverter::mock(),
        }
    }
}