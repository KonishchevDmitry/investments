use crate::currency::Cash;
use crate::time::Date;

pub struct Fee {
    pub date: Date,
    pub amount: Cash, // The amount is positive for commission and negative for refund
    pub description: Option<String>,
}

impl Fee {
    pub fn new(date: Date, amount: Cash, description: Option<String>) -> Fee {
        Fee {date, amount, description}
    }

    pub fn local_description(&self) -> &str {
        match self.description.as_ref() {
            Some(description) => description,
            None => if self.amount.is_negative() {
                "Возврат излишне удержанной комиссии"
            } else {
                "Комиссия брокера"
            },
        }
    }
}