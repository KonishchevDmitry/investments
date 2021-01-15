use crate::currency::Cash;
use crate::types::Date;

#[derive(Debug)]
pub struct Fee {
    pub date: Date,
    pub amount: Cash, // The amount is positive for commission and negative for refund
    pub description: Option<String>,
}