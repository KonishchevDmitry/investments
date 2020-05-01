use std::collections::{HashMap, HashSet};
use std::default::Default;

use chrono::Datelike;
use lazy_static::lazy_static;

use crate::core::EmptyResult;
use crate::currency;
use crate::formatting::format_date;
use crate::localities::Country;
use crate::types::{Date, Decimal};
use crate::util;

#[derive(Debug, Clone, Copy)]
pub enum TaxPaymentDay {
    Day {month: u32, day: u32},
    OnClose,
}

impl Default for TaxPaymentDay {
    fn default() -> TaxPaymentDay {
        TaxPaymentDay::Day {
            month: 3,
            day: 15,
        }
    }
}

impl TaxPaymentDay {
    /// Returns an approximate date when tax is going to be paid for the specified income
    pub fn get(&self, income_date: Date) -> Date {
        lazy_static! {
            static ref ACCOUNT_CLOSE_DATE: Date = Date::from_ymd(util::today().year() + 10, 1, 1);
        }

        match *self {
            TaxPaymentDay::Day {month, day} => Date::from_ymd(income_date.year() + 1, month, day),
            TaxPaymentDay::OnClose => *ACCOUNT_CLOSE_DATE,
        }
    }
}

pub struct TaxRemapping {
    remapping: HashMap<(Date, String), (Date, bool)>
}

impl TaxRemapping {
    pub fn new() -> TaxRemapping {
        TaxRemapping {
            remapping: HashMap::new(),
        }
    }

    pub fn add(&mut self, date: Date, description: &str, to_date: Date) -> EmptyResult {
        if self.remapping.insert((date, description.to_owned()), (to_date, false)).is_some() {
            return Err!(
                "Invalid tax remapping configuration: Duplicated match: {} - {:?}",
                format_date(date), description);
        }
        Ok(())
    }

    pub fn map(&mut self, date: Date, description: &str) -> Date {
        if let Some((to_date, mapped)) = self.remapping.get_mut(&(date, description.to_owned())) {
            *mapped = true;
            *to_date
        } else {
            date
        }
    }

    pub fn ensure_all_mapped(&self) -> EmptyResult {
        for ((date, description), (_, mapped)) in self.remapping.iter() {
            if !mapped {
                return Err!(
                    "The following tax remapping rule hasn't been mapped to any tax: {} - {:?}",
                    format_date(*date), description)
            }
        }

        Ok(())
    }
}

pub struct NetTaxCalculator {
    country: Country,
    tax_payment_day: TaxPaymentDay,
    profit: HashMap<Date, Decimal>,
}

impl NetTaxCalculator {
    pub fn new(country: Country, tax_payment_day: TaxPaymentDay) -> NetTaxCalculator {
        NetTaxCalculator {
            country,
            tax_payment_day,
            profit: HashMap::new(),
        }
    }

    pub fn add_profit(&mut self, date: Date, amount: Decimal) {
        let amount = currency::round(amount);
        self.profit.entry(self.tax_payment_day.get(date))
            .and_modify(|profit| *profit += amount)
            .or_insert(amount);
    }

    pub fn get_taxes(&self) -> HashMap<Date, Decimal> {
        let mut taxes = HashMap::new();
        let mut years = HashSet::new();

        for (&tax_payment_date, &profit) in self.profit.iter() {
            let year = tax_payment_date.year();
            assert!(years.insert(year)); // Ensure that we have only one tax payment date per year

            let tax_to_pay = self.country.tax_to_pay(profit, None);
            assert_eq!(taxes.insert(tax_payment_date, tax_to_pay), None);
        }

        taxes
    }
}