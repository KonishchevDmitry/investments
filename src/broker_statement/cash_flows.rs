use crate::currency::Cash;
use crate::time::{Date, DateOptTime};

// Represents actual cash flows on account including reversal operations. Used to be able to
// calculate cash balance for specific point of time.
pub struct CashFlow {
    pub date: DateOptTime,
    pub amount: Cash,
    pub type_: CashFlowType,
}

pub enum CashFlowType {
    Dividend {date: Date, issuer: String},
    Tax {date: Date, issuer: String},
}

impl CashFlow {
    pub fn new(date: DateOptTime, amount: Cash, type_: CashFlowType) -> CashFlow {
        CashFlow {date, amount, type_}
    }

    pub fn symbol(&self) -> Option<&str> {
        Some(match self.type_ {
            CashFlowType::Dividend {ref issuer, ..} => &issuer,
            CashFlowType::Tax {ref issuer, ..} => &issuer,
        })
    }

    pub fn mut_symbol(&mut self) -> Option<&mut String> {
        Some(match self.type_ {
            CashFlowType::Dividend {ref mut issuer, ..} => issuer,
            CashFlowType::Tax {ref mut issuer, ..} => issuer,
        })
    }

    pub fn sort_key(&self) -> (DateOptTime, Option<&str>, Date) {
        (self.date, self.symbol(), match self.type_ {
            CashFlowType::Dividend {date, ..} => date,
            CashFlowType::Tax {date, ..} => date,
        })
    }
}