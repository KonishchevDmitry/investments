use crate::currency::Cash;
use crate::types::Date;

#[derive(Debug)]
pub struct Fee {
    pub date: Date,
    pub amount: Cash, // The amount is negative for commission and positive for refund
    pub description: Option<String>,
}