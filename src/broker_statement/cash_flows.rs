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
    Repo {symbol: String, commission: Cash},
}

impl CashFlow {
    pub fn new(date: DateOptTime, amount: Cash, type_: CashFlowType) -> CashFlow {
        CashFlow {date, amount, type_}
    }

    pub fn symbol(&self) -> Option<&str> {
        Some(match &self.type_ {
            CashFlowType::Dividend {issuer, ..} => issuer,
            CashFlowType::Tax {issuer, ..} => issuer,
            CashFlowType::Repo {symbol, ..} => symbol,
        })
    }

    pub fn mut_symbol(&mut self) -> Option<&mut String> {
        Some(match &mut self.type_ {
            CashFlowType::Dividend {issuer, ..} => issuer,
            CashFlowType::Tax {issuer, ..} => issuer,
            CashFlowType::Repo {symbol, ..} => symbol,
        })
    }

    pub fn sort_key(&self) -> (DateOptTime, Option<&str>, Option<Date>) {
        (self.date, self.symbol(), match self.type_ {
            CashFlowType::Dividend {date, ..} => Some(date),
            CashFlowType::Tax {date, ..} => Some(date),
            CashFlowType::Repo {..} => None,
        })
    }
}