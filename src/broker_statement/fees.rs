use crate::currency::Cash;
use crate::time::{Date, DateOptTime};

#[derive(Debug)]
pub struct Fee {
    pub time: DateOptTime,
    pub date: Date, // FIXME(konishchev): Switch to DateOptTime
    pub amount: Cash, // The amount is positive for commission and negative for refund
    pub description: Option<String>,
}

impl Fee {
    pub fn new(date: Date, amount: Cash, description: Option<String>) -> Fee {
        Fee {time: date.into(), date, amount, description}
    }

    pub fn local_description(&self) -> &str {
        match self.description.as_ref() {
            Some(description) => &description,
            None => if self.amount.is_negative() {
                "Возврат излишне удержанной комиссии"
            } else {
                "Комиссия брокера"
            },
        }
    }
}