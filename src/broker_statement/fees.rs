use crate::currency::Cash;
use crate::types::Date;

#[derive(Debug)]
pub struct Fee {
    pub date: Date,
    pub amount: Cash,
}