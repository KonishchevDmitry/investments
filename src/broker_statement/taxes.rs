use crate::broker_statement::payments::Payments;
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