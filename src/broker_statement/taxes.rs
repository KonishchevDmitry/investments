use chrono::Datelike;

use crate::broker_statement::payments::Payments;
use crate::core::GenericResult;
use crate::currency::Cash;
use crate::formatting::format_date;
use crate::types::Date;

#[derive(PartialEq, Eq, Hash)]
pub struct TaxId {
    pub date: Date,
    pub issuer: String,
}

impl TaxId {
    pub fn new(date: Date, issuer: &str) -> TaxId {
        TaxId { date, issuer: issuer.to_owned() }
    }
}

pub type TaxAccruals = Payments;

#[derive(Debug)]
pub struct TaxWithholding {
    pub date: Date,
    pub year: i32,
    pub amount: Cash,
}

impl TaxWithholding {
    pub fn new(date: Date, year: i32, amount: Cash) -> GenericResult<TaxWithholding> {
        if year != date.year() && year != date.year() - 1 {
            return Err!("Got an unexpected {} year tax withholding on {}", year, format_date(date));
        }
        Ok(TaxWithholding {date, year, amount})
    }
}