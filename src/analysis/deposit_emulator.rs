use chrono::Datelike;

use crate::core::GenericResult;
#[cfg(test)] use crate::currency;
use crate::types::{Date, Decimal};

pub struct DepositEmulator {
    date: Date,
    end_date: Date,

    monthly_capitalization: bool,
    interest_periods: Vec<InterestPeriod>,
    interest_period: Option<ActiveInterestPeriod>,

    daily_interest: Decimal,
    assets: Decimal,
}

impl DepositEmulator {
    pub fn new(start_date: Date, end_date: Date, interest: Decimal) -> DepositEmulator {
        assert!(start_date <= end_date);

        let mut interest_periods = Vec::new();
        if start_date != end_date {
            interest_periods.push(InterestPeriod::new(start_date, end_date));
        }

        DepositEmulator {
            date: start_date,
            end_date: end_date,

            monthly_capitalization: true,
            interest_periods: interest_periods,
            interest_period: None,

            daily_interest: interest / dec!(100) / dec!(365),
            assets: dec!(0),
        }
    }

    pub fn with_monthly_capitalization(mut self, monthly_capitalization: bool) -> DepositEmulator {
        self.monthly_capitalization = monthly_capitalization;
        self
    }

    pub fn with_interest_periods(mut self, custom_interest_periods: &[InterestPeriod]) -> DepositEmulator {
        self.interest_periods = custom_interest_periods.iter().rev().cloned().collect();
        self
    }

    pub fn emulate(mut self, transactions: &[Transaction]) -> Decimal {
        self.select_interest_period();

        for transaction in transactions {
            self.process_transaction(transaction);
        }

        self.process_to(self.end_date);
        assert!(self.interest_period.is_none());

        self.assets
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
            monthly_capitalization: self.monthly_capitalization,
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

    pub fn days(&self) -> u32 {
        let days = (self.end - self.start).num_days();
        cast::u32(days).unwrap()
    }
}

#[derive(Clone, Copy)]
struct ActiveInterestPeriod {
    start_date: Date,
    monthly_capitalization: bool,
    next_capitalization_date: Date,
    accumulated_income: Decimal,
    end_date: Date,
}

impl ActiveInterestPeriod {
    fn set_next_capitalization_date(&mut self) {
        assert!(self.next_capitalization_date < self.end_date);

        if self.monthly_capitalization {
            self.next_capitalization_date = get_next_capitalization_date(
                self.next_capitalization_date, self.start_date.day()).unwrap();

            if self.next_capitalization_date > self.end_date {
                self.next_capitalization_date = self.end_date;
            }
        } else {
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

fn get_next_capitalization_date(current: Date, capitalization_day: u32) -> GenericResult<Date> {
    if current.day() != capitalization_day && !(
        current.day() < capitalization_day && current.succ().month() != current.month()
    ) {
        return Err!(
            "Got an unexpected current capitalization date for the specified capitalization day");
    }

    let (year, month) = get_next_year_month(current.year(), current.month());

    Ok(match Date::from_ymd_opt(year, month, capitalization_day) {
        Some(date) => date,
        None => {
            let (year, month) = get_next_year_month(year, month);
            let date = Date::from_ymd(year, month, 1).pred();
            let days = (date - current).num_days();
            assert!((28..=31).contains(&days));
            date
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_deposit() {
        let open_date = date!(2018, 7, 28);
        let interest = dec!(7);
        let transactions = vec![Transaction::new(open_date, dec!(600_000))];

        for &(capitalization_date, expected_assets) in &[
            (date!(2018,  7, 28), dec!(600_000.00)),
            (date!(2018,  8, 28), dec!(603_567.12)),
            (date!(2018,  9, 28), dec!(607_155.45)),
            (date!(2018, 10, 28), dec!(610_648.68)),
            (date!(2018, 11, 28), dec!(614_279.11)),
            (date!(2018, 12, 28), dec!(617_813.32)),
            (date!(2019,  1, 28), dec!(621_486.34)),
        ] {
            let result = DepositEmulator::new(open_date, capitalization_date, interest)
                .emulate(&transactions);
            assert_eq!(currency::round(result), expected_assets);

            {
                // Test deposit closing

                let mut transactions = transactions.clone();
                transactions.push(Transaction::new(capitalization_date, -expected_assets));

                let result = DepositEmulator::new(open_date, capitalization_date, interest)
                    .emulate(&transactions);
                assert_eq!(currency::round(result), dec!(0));
            }
        }
    }

    #[test]
    fn real_deposit_with_contributions() {
        let open_date = date!(2019, 1, 31);
        let interest = dec!(7);
        let transactions = vec![
            Transaction::new(open_date, dec!(190_000)),
            Transaction::new(date!(2019, 2,  5), dec!(60_000)),
            Transaction::new(date!(2019, 2, 21), dec!(50_000)),
        ];

        for &(capitalization_date, expected_assets) in &[
            (date!(2019, 2, 28), dec!(301_352.05)),
            (date!(2019, 3, 31), dec!(303_143.65)),
            (date!(2019, 4, 30), dec!(304_887.77)),
            (date!(2019, 5, 31), dec!(306_700.39)),
            (date!(2019, 6, 30), dec!(308_464.97)),
            (date!(2019, 7, 31), dec!(310_298.85)),
        ] {
            let result = DepositEmulator::new(open_date, capitalization_date, interest)
                .emulate(&transactions);
            assert_eq!(currency::round(result), expected_assets);
        }
    }

    #[test]
    fn joint_deposits() {
        let open_date = date!(2018, 1, 1);
        let interest = dec!(7);

        // Some assets without interest
        let mut transactions = vec![Transaction::new(open_date, dec!(200_000))];
        let mut interest_periods = Vec::new();

        // First deposit
        transactions.push(Transaction::new(date!(2018, 7, 28), dec!(400_000)));
        interest_periods.push(InterestPeriod::new(date!(2018, 7, 28), date!(2019, 1, 28)));
        let result = DepositEmulator::new(open_date, date!(2019, 1, 28), interest)
            .with_interest_periods(&interest_periods)
            .emulate(&transactions);
        assert_eq!(currency::round(result), dec!(621_486.34));

        // A pause with no interest
        transactions.push(Transaction::new(date!(2019, 1, 28), dec!(100_000) - result));
        transactions.push(Transaction::new(date!(2019, 1, 31), dec!(90_000)));
        let result = DepositEmulator::new(open_date, date!(2019, 1, 31), interest)
            .with_interest_periods(&interest_periods)
            .emulate(&transactions);
        assert_eq!(currency::round(result), dec!(190_000));

        // Second deposit
        interest_periods.push(InterestPeriod::new(date!(2019, 1, 31), date!(2019, 7, 31)));
        let result = DepositEmulator::new(open_date, date!(2019, 7, 31), interest)
            .with_interest_periods(&interest_periods)
            .emulate(&transactions);
        assert_eq!(currency::round(result), dec!(196_691.45));

        transactions.push(Transaction::new(date!(2019, 2, 5), dec!(60_000)));
        let result = DepositEmulator::new(open_date, date!(2019, 7, 31), interest)
            .with_interest_periods(&interest_periods)
            .emulate(&transactions);
        assert_eq!(currency::round(result), dec!(258_745.30));

        transactions.push(Transaction::new(date!(2019, 2, 21), dec!(50_000)));
        let result = DepositEmulator::new(open_date, date!(2019, 7, 31), interest)
            .with_interest_periods(&interest_periods)
            .emulate(&transactions);
        assert_eq!(currency::round(result), dec!(310_298.85));

        // Some activity with no interest
        transactions.push(Transaction::new(date!(2019, 7, 31), dec!(100_000) - result));
        let result = DepositEmulator::new(open_date, date!(2020, 1, 1), interest)
            .with_interest_periods(&interest_periods)
            .emulate(&transactions);
        assert_eq!(currency::round(result), dec!(100_000));
    }

    #[test]
    fn deposit_without_monthly_capitalization() {
        let open_date = date!(2018, 7, 28);
        let interest = dec!(6);

        let transactions = vec![
            Transaction::new(open_date, dec!(100_000)),
            Transaction::new(date!(2018, 8, 10), dec!(100_000)),
        ];

        for &(capitalization_date, expected_assets) in &[
            (date!(2018,  8, 28), dec!(200_805.48)),
            (date!(2018,  9, 28), dec!(201_824.66)),
            (date!(2018, 10, 28), dec!(202_810.96)),
            (date!(2018, 11, 28), dec!(203_830.14)),
            (date!(2018, 12, 28), dec!(204_816.44)),
            (date!(2019,  1, 28), dec!(205_835.62)),
        ] {
            let result = DepositEmulator::new(open_date, capitalization_date, interest)
                .with_monthly_capitalization(false)
                .emulate(&transactions);
            assert_eq!(currency::round(result), expected_assets);

            {
                // Test deposit closing

                let mut transactions = transactions.clone();
                transactions.push(Transaction::new(capitalization_date, -expected_assets));

                let result = DepositEmulator::new(open_date, capitalization_date, interest)
                    .with_monthly_capitalization(false)
                    .emulate(&transactions);
                assert_eq!(currency::round(result), dec!(0));
            }
        }
    }

    #[test]
    fn next_capitalization_date() {
        // Dec -> Jan
        for day in 1..32 {
            assert_eq!(get_next_capitalization_date(date!(2018, 12, day), day).unwrap(),
                       date!(2019, 1, day));
        }

        // Jan -> Feb
        for day in 1..29 {
            assert_eq!(get_next_capitalization_date(date!(2019, 1, day), day).unwrap(),
                       date!(2019, 2, day));
        }
        for day in 29..32 {
            assert_eq!(get_next_capitalization_date(date!(2019, 1, day), day).unwrap(),
                       date!(2019, 2, 28));
        }

        // Feb -> Mar
        for day in 1..29 {
            assert_eq!(get_next_capitalization_date(date!(2019, 2, day), day).unwrap(),
                       date!(2019, 3, day));
        }
        for day in 28..32 {
            assert_eq!(get_next_capitalization_date(date!(2019, 2, 28), day).unwrap(),
                       date!(2019, 3, day));
        }
        for day in 1..28 {
            assert!(get_next_capitalization_date(date!(2019, 2, 28), day).is_err());
        }
    }
}