use std::collections::{HashMap, HashSet};
use std::default::Default;

use chrono::Datelike;
use regex::Regex;
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::core::EmptyResult;
use crate::currency;
use crate::formatting::format_date;
use crate::localities::{self, Country, Jurisdiction};
use crate::time;
use crate::types::{Date, Decimal};

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
    pub fn is_applicable(&self) -> (bool, bool) {
        match self {
            TaxExemption::TaxFree => (true, true),
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

pub struct TaxPaymentDay {
    jurisdiction: Jurisdiction,
    pub spec: TaxPaymentDaySpec,
}

impl TaxPaymentDay {
    pub fn new(jurisdiction: Jurisdiction, spec: TaxPaymentDaySpec) -> TaxPaymentDay {
        TaxPaymentDay {jurisdiction, spec}
    }

    /// Returns tax year and an approximate date when tax is going to be paid for the specified income
    pub fn get(&self, income_date: Date, trading: bool) -> (i32, Date) {
        match self.spec {
            TaxPaymentDaySpec::Day {mut month, mut day} => {
                let tax_year = income_date.year();

                if trading && self.jurisdiction == Jurisdiction::Russia {
                    month = 1;
                    day = 1;
                }

                (tax_year, Date::from_ymd(tax_year + 1, month, day))
            },

            TaxPaymentDaySpec::OnClose(close_date) => {
                assert!(income_date <= close_date);

                if trading {
                    (close_date.year(), close_date)
                } else {
                    let day = TaxPaymentDaySpec::default();
                    let tax_payment_day = TaxPaymentDay::new(self.jurisdiction, day);
                    tax_payment_day.get(income_date, trading)
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TaxPaymentDaySpec {
    Day {month: u32, day: u32},
    OnClose(Date),
}

impl Default for TaxPaymentDaySpec {
    fn default() -> TaxPaymentDaySpec {
        TaxPaymentDaySpec::Day {
            month: 3,
            day: 15,
        }
    }
}

impl TaxPaymentDaySpec {
    pub fn deserialize<'de, D>(deserializer: D) -> Result<TaxPaymentDaySpec, D::Error>
        where D: Deserializer<'de>
    {
        let tax_payment_day: String = Deserialize::deserialize(deserializer)?;
        if tax_payment_day == "on-close" {
            return Ok(TaxPaymentDaySpec::OnClose(localities::nearest_possible_account_close_date()));
        }

        Regex::new(r"^(?P<day>[0-9]+)\.(?P<month>[0-9]+)$").unwrap().captures(&tax_payment_day).and_then(|captures| {
            let day = captures.name("day").unwrap().as_str().parse::<u32>().ok();
            let month = captures.name("month").unwrap().as_str().parse::<u32>().ok();
            let (day, month) = match (day, month) {
                (Some(day), Some(month)) => (day, month),
                _ => return None,
            };

            if Date::from_ymd_opt(time::today().year(), month, day).is_none() || (day, month) == (29, 2) {
                return None;
            }

            Some(TaxPaymentDaySpec::Day {month, day})
        }).ok_or_else(|| D::Error::custom(format!("Invalid tax payment day: {:?}", tax_payment_day)))
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
    profit: HashMap<(i32, Date), NetProfit>,
}

#[derive(Default)]
struct NetProfit {
    total: Decimal,
    taxable: Decimal,
}

impl NetTaxCalculator {
    pub fn new(country: Country, tax_payment_day: TaxPaymentDay) -> NetTaxCalculator {
        NetTaxCalculator {
            country,
            tax_payment_day,
            profit: HashMap::new(),
        }
    }

    pub fn add_profit(&mut self, date: Date, total: Decimal, taxable: Decimal) {
        let total = currency::round(total);
        let taxable = currency::round(taxable);
        let key = self.tax_payment_day.get(date, true);

        let profit = self.profit.entry(key).or_default();
        profit.total += total;
        profit.taxable += taxable;
    }

    pub fn get_taxes(&self) -> HashMap<Date, (Decimal, Decimal)> {
        let mut taxes = HashMap::new();
        let mut years = HashSet::new();

        for (&(tax_year, tax_payment_date), profit) in self.profit.iter() {
            assert!(years.insert(tax_year)); // Ensure that we have only one tax payment per year

            let tax_to_pay = self.country.tax_to_pay(
                IncomeType::Trading, tax_year, profit.taxable, None);

            let tax_without_deduction = self.country.tax_to_pay(
                IncomeType::Trading, tax_year, profit.total, None);

            let tax_deduction = tax_without_deduction - tax_to_pay;
            assert!(!tax_deduction.is_sign_negative());

            assert_eq!(taxes.insert(tax_payment_date, (tax_to_pay, tax_deduction)), None);
        }

        taxes
    }
}