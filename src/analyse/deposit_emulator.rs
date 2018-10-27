use chrono::{Duration, Datelike};

use core::{EmptyResult, GenericResult};
#[cfg(test)] use currency;
use currency::CashAssets;
use currency::converter::CurrencyConverter;
#[cfg(test)] use currency::converter::CurrencyConverterBackend;
use types::{Date, Decimal};
use util;

pub struct DepositEmulator<'a> {
    date: Date,
    capitalization_day: u32,
    next_capitalization_date: Date,

    assets: Decimal,
    accumulated_income: Decimal,

    currency: &'a str,
    daily_interest: Decimal,
    converter: &'a CurrencyConverter,
}

impl<'a> DepositEmulator<'a> {
    pub fn emulate(
        start_date: Date, start_assets: Decimal, transactions: &Vec<CashAssets>, end_date: Date,
        currency: &str, interest: Decimal, converter: &CurrencyConverter,
    ) -> GenericResult<Decimal> {
        let mut emulator = DepositEmulator {
            date: start_date,
            capitalization_day: start_date.day(),
            next_capitalization_date: start_date,

            assets: start_assets,
            accumulated_income: dec!(0),

            currency: currency,
            daily_interest: interest / dec!(100) / dec!(365),
            converter: converter,
        };
        emulator.set_next_capitalization_date();

        for transaction in transactions {
            emulator.process_transaction(transaction)?;
        }

        emulator.process_to(end_date)?;
        emulator.capitalize()?;

        Ok(emulator.assets)
    }

    fn process_transaction(&mut self, transaction: &CashAssets) -> EmptyResult {
        self.process_to(transaction.date)?;
        assert_eq!(self.date, transaction.date);

        self.assets += self.converter.convert_to(
            transaction.date, transaction.cash, self.currency)?;

        if self.assets < dec!(0) {
            return Err!("Portfolio got negative balance on {}", util::format_date(transaction.date));
        }

        Ok(())
    }

    fn process_to(&mut self, date: Date) -> EmptyResult {
        while date >= self.next_capitalization_date {
            let capitalization_date = self.next_capitalization_date;
            self.accumulate_income(capitalization_date)?;
            self.capitalize()?;
            self.set_next_capitalization_date();
        }

        self.accumulate_income(date)?;

        Ok(())
    }

    fn accumulate_income(&mut self, date: Date) -> EmptyResult {
        assert!(self.date <= date);
        assert!(date <= self.next_capitalization_date);

        let days = (date - self.date).num_days();
        self.accumulated_income += self.assets * self.daily_interest * Decimal::from(days);
        self.date = date;

        Ok(())
    }

    fn capitalize(&mut self) -> EmptyResult {
        self.assets += self.accumulated_income;
        self.accumulated_income = dec!(0);

        if self.assets < dec!(0) {
            return Err!("Portfolio got negative balance on {}", util::format_date(self.date));
        }

        Ok(())
    }

    fn set_next_capitalization_date(&mut self) {
        assert_eq!(self.date, self.next_capitalization_date);
        self.next_capitalization_date = get_next_capitalization_date(
            self.next_capitalization_date, self.capitalization_day);
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

    macro_rules! deposit_emulator_tests {
        ($($name:ident: $args:expr,)*) => {
        $(
            #[test]
            fn $name() {
                let (transaction_currency, transaction_amount) = $args;
                deposit_emulator(transaction_currency, transaction_amount);
            }
        )*
        }
    }

    deposit_emulator_tests! {
        deposit_emulator_rub: ("RUB", dec!(400000)),
        deposit_emulator_usd: ("USD", dec!(4000)),
    }

    fn deposit_emulator(transaction_currency: &str, transaction_amount: Decimal) {
        struct ConverterMock {}
        impl CurrencyConverterBackend for ConverterMock {
            fn convert(&self, from: &str, to: &str, _date: Date, amount: Decimal) -> GenericResult<Decimal> {
                Ok(match (from, to) {
                    ("RUB", "RUB") => amount,
                    ("USD", "RUB") => amount * dec!(100),
                    _ => unreachable!(),
                })
            }
        }
        let converter = CurrencyConverter::new_with_backend(Box::new(ConverterMock {}));

        let start_date = date!(28, 7, 2018);
        let initial_assets = dec!(200000);
        let currency = "RUB";
        let interest = dec!(7);

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &vec![CashAssets::new(date!(28, 7, 2018), transaction_currency, transaction_amount)],
            date!(28, 9, 2018), currency, interest, &converter).unwrap();
        assert_eq!(currency::round(result), decs!("607155.45"));

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &vec![CashAssets::new(date!(28, 8, 2018), transaction_currency, transaction_amount)],
            date!(28, 9, 2018), currency, interest, &converter).unwrap();
        assert_eq!(currency::round(result), decs!("604763.23"));

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &vec![CashAssets::new(date!(14, 8, 2018), transaction_currency, transaction_amount)],
            date!(28, 9, 2018), currency, interest, &converter).unwrap();
        assert_eq!(currency::round(result), decs!("605843.59"));

        let result = DepositEmulator::emulate(
            start_date, initial_assets,
            &vec![CashAssets::new(date!(28, 7, 2018), transaction_currency, transaction_amount)],
            date!(28, 1, 2019), currency, interest, &converter).unwrap();
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