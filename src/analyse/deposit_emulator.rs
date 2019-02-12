use chrono::{Duration, Datelike};

#[cfg(test)] use crate::currency;
use crate::types::{Date, Decimal};

pub struct DepositEmulator {
    date: Date,
    interest_periods: Vec<InterestPeriod>,
    interest_period: Option<ActiveInterestPeriod>,
    daily_interest: Decimal,
    assets: Decimal,
}

impl DepositEmulator {
    pub fn emulate(
        start_date: Date, start_assets: Decimal, transactions: &[Transaction], end_date: Date,
        interest: Decimal, custom_interest_periods: Option<&[InterestPeriod]>
    ) -> Decimal {
        let mut interest_periods = Vec::new();

        if let Some(custom_interest_periods) = custom_interest_periods {
            interest_periods.extend(custom_interest_periods.iter().rev())
        } else if start_date != end_date {
            interest_periods.push(InterestPeriod::new(start_date, end_date));
        } else {
            assert!(start_date <= end_date);
        }

        let mut emulator = DepositEmulator {
            date: start_date,
            interest_periods: interest_periods,
            interest_period: None,
            daily_interest: interest / dec!(100) / dec!(365),
            assets: start_assets,
        };
        emulator.select_interest_period();

        for transaction in transactions {
            emulator.process_transaction(transaction);
        }

        emulator.process_to(end_date);
        assert!(emulator.interest_period.is_none());

        emulator.assets
    }

    fn select_interest_period(&mut self) {
        assert!(self.interest_period.is_none());

        let period = match self.interest_periods.last() {
            Some(period) => *period,
            None => return,
        };

        assert!(self.date <= period.start);
        if self.date != period.start {
            return
        }

        self.interest_periods.pop().unwrap();

        let mut interest_period = ActiveInterestPeriod {
            start_date: period.start,
            next_capitalization_date: period.start,
            accumulated_income: dec!(0),
            end_date: period.end,
        };
        interest_period.set_next_capitalization_date();

        self.interest_period = Some(interest_period);
    }

    fn process_transaction(&mut self, transaction: &Transaction) {
        self.process_to(transaction.date);
        self.assets += transaction.amount;
    }

    fn process_to(&mut self, date: Date) {
        assert!(self.date <= date);

        while self.date < date {
            if let Some(interest_period) = self.interest_period {
                // We're inside of the interest period

                if date >= interest_period.next_capitalization_date {
                    self.accumulate_income_to(interest_period.next_capitalization_date);

                    if self.date == interest_period.end_date {
                        self.close_interest_period();
                    } else {
                        self.capitalize();
                    }
                } else {
                    self.accumulate_income_to(date);
                }
            } else {
                // We're outside of the interest period

                if let Some(next_period) = self.interest_periods.last() {
                    assert!(self.date < next_period.start);

                    if date < next_period.start {
                        self.date = date;
                    } else {
                        self.date = next_period.start;
                        self.select_interest_period();
                    }
                } else {
                    self.date = date;
                }
            }
        }

        assert_eq!(self.date, date);
    }

    fn accumulate_income_to(&mut self, date: Date) {
        let interest_period = self.interest_period.as_mut().unwrap();

        assert!(self.date <= date);
        assert!(interest_period.start_date <= self.date);
        assert!(date <= interest_period.next_capitalization_date);

        if self.assets.is_sign_positive() {
            let days = (date - self.date).num_days();
            let income = self.assets * self.daily_interest * Decimal::from(days);
            interest_period.accumulated_income += income;
        }

        self.date = date;
    }

    fn capitalize(&mut self) {
        let interest_period = self.interest_period.as_mut().unwrap();
        assert_eq!(self.date, interest_period.next_capitalization_date);

        self.assets += interest_period.accumulated_income;
        interest_period.accumulated_income = dec!(0);

        interest_period.set_next_capitalization_date();
    }

    fn close_interest_period(&mut self) {
        let interest_period = self.interest_period.take().unwrap();
        assert_eq!(self.date, interest_period.end_date);
        self.assets += interest_period.accumulated_income;

        self.select_interest_period();
    }
}

#[cfg_attr(test, derive(Clone, Copy))]
pub struct Transaction {
    pub date: Date,
    pub amount: Decimal,
}

impl Transaction {
    pub fn new(date: Date, amount: Decimal) -> Transaction {
        Transaction {
            date: date,
            amount: amount,
        }
    }
}

#[derive(Clone, Copy)]
pub struct InterestPeriod {
    pub start: Date,
    pub end: Date,
}

impl InterestPeriod {
    pub fn new(start: Date, end: Date) -> InterestPeriod {
        assert!(start < end);
        InterestPeriod { start, end }
    }
}

#[derive(Clone, Copy)]
struct ActiveInterestPeriod {
    start_date: Date,
    next_capitalization_date: Date,
    accumulated_income: Decimal,
    end_date: Date,
}

impl ActiveInterestPeriod {
    fn set_next_capitalization_date(&mut self) {
        assert!(self.next_capitalization_date < self.end_date);

        self.next_capitalization_date = get_next_capitalization_date(
            self.next_capitalization_date, self.start_date.day());

        if self.next_capitalization_date > self.end_date {
            self.next_capitalization_date = self.end_date;
        }
    }
}

fn get_next_year_month(mut year: i32, mut month: u32) -> (i32, u32) {
    if month == 12 {
        year += 1;
        month = 1;
    } else {
        month += 1;
    }

    (year, month)
}

fn get_next_capitalization_date(current: Date, capitalization_day: u32) -> Date {
    let (year, month) = if current.day() == capitalization_day {
        get_next_year_month(current.year(), current.month())
    } else {
        assert!(
            current.day() == 1 &&
                (current - Duration::days(1)).day() < capitalization_day
        );
        (current.year(), current.month())
    };

    match Date::from_ymd_opt(year, month, capitalization_day) {
        Some(date) => date,
        None => {
            let (year, month) = get_next_year_month(year, month);
            let date = Date::from_ymd(year, month, 1);
            let days = (date - current).num_days();
            assert!(days >= 29 && days <= 31);
            date
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_emulator() {
        // FIXME: Replace with real deposit with contributions

        let start_date = date!(28, 7, 2018);
        let initial_assets = dec!(200_000);
        let transaction_amount = dec!(400_000);
        let interest = dec!(7);

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &[Transaction::new(date!(28, 8, 2018), transaction_amount)],
            date!(28, 9, 2018), interest, None);
        assert_eq!(currency::round(result), dec!(604_763.23));

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &[Transaction::new(date!(14, 8, 2018), transaction_amount)],
            date!(28, 9, 2018), interest, None);
        assert_eq!(currency::round(result), dec!(605_843.59));
    }

    #[test]
    fn real_deposit() {
        test_real_deposit(dec!(600_000), None);
    }

    #[test]
    fn real_deposit_fake_transaction() {
        test_real_deposit(dec!(0), Some(dec!(600_000)));
    }

    #[test]
    fn real_deposit_initial_and_fake_transaction() {
        test_real_deposit(dec!(400_000), Some(dec!(200_000)));
    }

    fn test_real_deposit(initial_assets: Decimal, transaction_amount: Option<Decimal>) {
        let start_date = date!(28, 7, 2018);
        let interest = dec!(7);

        let mut transactions = Vec::new();
        if let Some(amount) = transaction_amount {
            transactions.push(Transaction::new(start_date, amount));
        }

        for (end_date, expected_assets) in [
            (date!(28,  7, 2018), dec!(600_000)),
            (date!(28,  8, 2018), dec!(603_567.12)),
            (date!(28,  9, 2018), dec!(607_155.45)),
            (date!(28, 10, 2018), dec!(610_648.68)),
            (date!(28, 11, 2018), dec!(614_279.11)),
            (date!(28, 12, 2018), dec!(617_813.32)),
            (date!(28,  1, 2019), dec!(621_486.34)),
        ].iter().cloned() {
            let result = DepositEmulator::emulate(
                start_date, initial_assets, &transactions, end_date, interest, None);
            assert_eq!(currency::round(result), expected_assets);

            {
                // Test deposit closing

                let mut transactions = transactions.clone();
                transactions.push(Transaction::new(end_date, -expected_assets));

                let result = DepositEmulator::emulate(
                    start_date, initial_assets, &transactions, end_date, interest, None);
                assert_eq!(currency::round(result), dec!(0));
            }
        }
    }

    #[test]
    fn joint_deposits() {
        let interest = dec!(7);
        let mut transactions = Vec::new();
        let mut interest_periods = Vec::new();

        // Some assets without interest
        let start_date = date!(1, 1, 2018);
        let initial_assets = dec!(200_000);

        // First deposit
        transactions.push(Transaction::new(date!(28, 7, 2018), dec!(400_000)));
        interest_periods.push(InterestPeriod::new(date!(28, 7, 2018), date!(28, 1, 2019)));
        let result = DepositEmulator::emulate(
            start_date, initial_assets, &transactions, date!(28, 1, 2019),
            interest, Some(&interest_periods));
        assert_eq!(currency::round(result), dec!(621_486.34));

        // A pause with no interest
        transactions.push(Transaction::new(date!(28, 1, 2019), dec!(100_000) - result));
        transactions.push(Transaction::new(date!(31, 1, 2019), dec!(90_000)));
        let result = DepositEmulator::emulate(
            start_date, initial_assets, &transactions, date!(31, 1, 2019),
            interest, Some(&interest_periods));
        assert_eq!(currency::round(result), dec!(190_000));

        // Second deposit
        interest_periods.push(InterestPeriod::new(date!(31, 1, 2019), date!(31, 7, 2019)));
        let result = DepositEmulator::emulate(
            start_date, initial_assets, &transactions, date!(31, 7, 2019),
            interest, Some(&interest_periods));
        assert_eq!(currency::round(result), dec!(196_691.46)); // FIXME: 46 -> 45

        // FIXME: Add contribution
        transactions.push(Transaction::new(date!(5, 2, 2019), dec!(60_000)));
        let result = DepositEmulator::emulate(
            start_date, initial_assets, &transactions, date!(31, 7, 2019),
            interest, Some(&interest_periods));
        assert_eq!(currency::round(result), dec!(258_745.33)); // FIXME: 33 -> 30

        // Some activity with no interest
        transactions.push(Transaction::new(date!(31, 7, 2019), dec!(100_000) - result));
        let result = DepositEmulator::emulate(
            start_date, initial_assets, &transactions, date!(1, 1, 2020),
            interest, Some(&interest_periods));
        assert_eq!(currency::round(result), dec!(100_000));
    }

    #[test]
    fn next_capitalization_date() {
        assert_eq!(get_next_capitalization_date(date!(1, 3, 2018), 1), date!(1, 4, 2018));
        assert_eq!(get_next_capitalization_date(date!(1, 3, 2018), 29), date!(29, 3, 2018));
        assert_eq!(get_next_capitalization_date(date!(1, 3, 2018), 31), date!(31, 3, 2018));
        assert_eq!(get_next_capitalization_date(date!(31, 3, 2018), 31), date!(1, 5, 2018));
    }
}