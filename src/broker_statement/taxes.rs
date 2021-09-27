use chrono::Datelike;

use crate::broker_statement::payments::Payments;
use crate::core::GenericResult;
use crate::currency::Cash;
use crate::formatting::format_date;
use crate::instruments::InstrumentId;
use crate::types::Date;

#[derive(PartialEq, Eq, Hash)]
pub struct TaxId {
    pub date: Date,
    pub issuer: InstrumentId,
}

impl TaxId {
    pub fn new(date: Date, issuer: InstrumentId) -> TaxId {
        TaxId {date, issuer}
    }
}

pub type TaxAccruals = Payments;

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