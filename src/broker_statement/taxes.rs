use crate::broker_statement::payments::Payments;
use crate::types::Date;

#[derive(PartialEq, Eq, Hash)]
pub struct TaxId {
    pub date: Date,
    pub description: String,
}

impl TaxId {
    pub fn new(date: Date, description: &str) -> TaxId {
        TaxId { date, description: description.to_owned() }
    }
}

pub type TaxAccruals = Payments;