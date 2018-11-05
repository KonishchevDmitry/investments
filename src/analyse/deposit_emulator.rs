use chrono::{Duration, Datelike};

#[cfg(test)] use currency;
use types::{Date, Decimal};

pub struct DepositEmulator {
    date: Date,
    capitalization_day: u32,
    next_capitalization_date: Date,

    assets: Decimal,
    daily_interest: Decimal,
    accumulated_income: Decimal,
}

impl DepositEmulator {
    pub fn emulate(
        start_date: Date, start_assets: Decimal, transactions: &Vec<Transaction>, end_date: Date,
        interest: Decimal,
    ) -> Decimal {
        let mut emulator = DepositEmulator {
            date: start_date,
            capitalization_day: start_date.day(),
            next_capitalization_date: start_date,

            assets: start_assets,
            daily_interest: interest / dec!(100) / dec!(365),
            accumulated_income: dec!(0),
        };
        emulator.set_next_capitalization_date();

        for transaction in transactions {
            emulator.process_transaction(transaction);
        }

        emulator.process_to(end_date);
        emulator.capitalize();

        emulator.assets
    }

    fn process_transaction(&mut self, transaction: &Transaction) {
        self.process_to(transaction.date);
        assert_eq!(self.date, transaction.date);
        self.assets += transaction.amount;
    }

    fn process_to(&mut self, date: Date) {
        while date >= self.next_capitalization_date {
            let capitalization_date = self.next_capitalization_date;
            self.accumulate_income(capitalization_date);
            self.capitalize();
            self.set_next_capitalization_date();
        }

        self.accumulate_income(date);
    }

    fn accumulate_income(&mut self, date: Date) {
        assert!(self.date <= date);
        assert!(date <= self.next_capitalization_date);

        if self.assets.is_sign_positive() {
            let days = (date - self.date).num_days();
            self.accumulated_income += self.assets * self.daily_interest * Decimal::from(days);
        }

        self.date = date;
    }

    fn capitalize(&mut self) {
        self.assets += self.accumulated_income;
        self.accumulated_income = dec!(0);
    }

    fn set_next_capitalization_date(&mut self) {
        assert_eq!(self.date, self.next_capitalization_date);
        self.next_capitalization_date = get_next_capitalization_date(
            self.next_capitalization_date, self.capitalization_day);
    }
}

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
        let start_date = date!(28, 7, 2018);
        let initial_assets = dec!(200000);
        let transaction_amount = dec!(400000);
        let interest = dec!(7);

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &vec![Transaction::new(date!(28, 7, 2018), transaction_amount)],
            date!(28, 9, 2018), interest);
        assert_eq!(currency::round(result), decs!("607155.45"));

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &vec![Transaction::new(date!(28, 8, 2018), transaction_amount)],
            date!(28, 9, 2018), interest);
        assert_eq!(currency::round(result), decs!("604763.23"));

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &vec![Transaction::new(date!(14, 8, 2018), transaction_amount)],
            date!(28, 9, 2018), interest);
        assert_eq!(currency::round(result), decs!("605843.59"));

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &vec![Transaction::new(date!(28, 7, 2018), transaction_amount)],
            date!(28, 1, 2019), interest);
        assert_eq!(currency::round(result), decs!("621486.34"));
    }

    #[test]
    fn next_capitalization_date() {
        assert_eq!(get_next_capitalization_date(date!(1, 3, 2018), 1), date!(1, 4, 2018));
        assert_eq!(get_next_capitalization_date(date!(1, 3, 2018), 29), date!(29, 3, 2018));
        assert_eq!(get_next_capitalization_date(date!(1, 3, 2018), 31), date!(31, 3, 2018));
        assert_eq!(get_next_capitalization_date(date!(31, 3, 2018), 31), date!(1, 5, 2018));
    }
}