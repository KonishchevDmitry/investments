use std::default::Default;

use chrono::Datelike;
use regex::Regex;
use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::localities::{self, Jurisdiction};
use crate::time;
use crate::types::Date;

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
        let tax_year = match self.spec {
            TaxPaymentDaySpec::Day {..} => income_date.year(),
            TaxPaymentDaySpec::OnClose(close_date) => {
                assert!(income_date <= close_date);

                if trading {
                    close_date.year()
                } else {
                    income_date.year()
                }
            },
        };
        (tax_year, self.get_for(tax_year, trading))
    }

    pub fn get_for(&self, tax_year: i32, trading: bool) -> Date {
        match self.spec {
            TaxPaymentDaySpec::Day {mut month, mut day} => {
                if trading && self.jurisdiction == Jurisdiction::Russia {
                    month = 1;
                    day = 1;
                }
                date!(tax_year + 1, month, day)
            },

            TaxPaymentDaySpec::OnClose(close_date) => {
                assert!(tax_year <= close_date.year());

                if trading {
                    close_date
                } else {
                    let spec = TaxPaymentDaySpec::default();
                    let tax_payment_day = TaxPaymentDay::new(self.jurisdiction, spec);
                    tax_payment_day.get_for(tax_year, trading)
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
            return Ok(TaxPaymentDaySpec::OnClose(localities::get_nearest_possible_russian_account_close_date()));
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
        }).ok_or_else(|| D::Error::custom(format!("Invalid tax payment day: {tax_payment_day:?}")))
    }
}