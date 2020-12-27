use std::collections::{HashMap, HashSet};
use std::default::Default;

use chrono::Datelike;
use regex::Regex;
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::EmptyResult;
use crate::currency;
use crate::formatting::format_date;
use crate::localities::{self, Country};
use crate::types::{Date, Decimal};
use crate::util;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum IncomeType {
    Trading,
    Dividends,
    Interest,
}

#[derive(Clone, Copy, Debug)]
pub enum TaxExemption {
    TaxFree,
}

impl TaxExemption {
    pub fn is_applicable(&self) -> bool {
        match self {
            TaxExemption::TaxFree => true,
        }
    }
}

impl<'de> Deserialize<'de> for TaxExemption {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "tax-free" => TaxExemption::TaxFree,
            _ => return Err(D::Error::unknown_variant(&value, &["tax-free"])),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TaxPaymentDay {
    Day {month: u32, day: u32},
    OnClose(Date),
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
    /// Returns tax year and an approximate date when tax is going to be paid for the specified income
    pub fn get(&self, income_date: Date, trading: bool) -> (i32, Date) {
        match *self {
            TaxPaymentDay::Day {month, day} => {
                (income_date.year(), Date::from_ymd(income_date.year() + 1, month, day))
            },
            TaxPaymentDay::OnClose(close_date) => {
                assert!(income_date <= close_date);

                if trading {
                    (close_date.year(), close_date)
                } else {
                    TaxPaymentDay::default().get(income_date, trading)
                }
            },
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<TaxPaymentDay, D::Error>
        where D: Deserializer<'de>
    {
        let tax_payment_day: String = Deserialize::deserialize(deserializer)?;
        if tax_payment_day == "on-close" {
            return Ok(TaxPaymentDay::OnClose(localities::nearest_possible_account_close_date()));
        }

        Ok(Regex::new(r"^(?P<day>[0-9]+)\.(?P<month>[0-9]+)$").unwrap().captures(&tax_payment_day).and_then(|captures| {
            let day = captures.name("day").unwrap().as_str().parse::<u32>().ok();
            let month = captures.name("month").unwrap().as_str().parse::<u32>().ok();
            let (day, month) = match (day, month) {
                (Some(day), Some(month)) => (day, month),
                _ => return None,
            };

            if Date::from_ymd_opt(util::today().year(), month, day).is_none() || (day, month) == (29, 2) {
                return None;
            }

            Some(TaxPaymentDay::Day {month, day})
        }).ok_or_else(|| D::Error::custom(format!("Invalid tax payment day: {:?}", tax_payment_day)))?)
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
    profit: HashMap<(i32, Date), Decimal>,
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
        self.profit.entry(self.tax_payment_day.get(date, true))
            .and_modify(|profit| *profit += amount)
            .or_insert(amount);
    }

    pub fn get_taxes(&self) -> HashMap<Date, Decimal> {
        let mut taxes = HashMap::new();
        let mut years = HashSet::new();

        for (&(tax_year, tax_payment_date), &profit) in self.profit.iter() {
            assert!(years.insert(tax_year)); // Ensure that we have only one tax payment per year

            let tax_to_pay = self.country.tax_to_pay(IncomeType::Trading, tax_year, profit, None);
            assert_eq!(taxes.insert(tax_payment_date, tax_to_pay), None);
        }

        taxes
    }
}