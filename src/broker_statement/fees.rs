use crate::broker_statement::payments::Withholding;
use crate::time::Date;

pub struct Fee {
    pub date: Date,
    pub amount: Withholding,
    pub description: Option<String>,
}

impl Fee {
    pub fn new(date: Date, amount: Withholding, description: Option<String>) -> Fee {
        Fee {date, amount, description}
    }

    pub fn local_description(&self) -> &str {
        match self.description.as_ref() {
            Some(description) => description,
            None => match self.amount {
                Withholding::Withholding(_) => "Комиссия брокера",
                Withholding::Refund(_) => "Возврат излишне удержанной комиссии",
            },
        }
    }
}