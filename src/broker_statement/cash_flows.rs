use crate::currency::Cash;
use crate::time::DateOptTime;

// Represents actual cash flows on account including reversal operations. Used to be able to
// calculate cash balance for specific point of time.
pub struct CashFlow {
    pub date: DateOptTime,
    pub amount: Cash,
    pub type_: CashFlowType,
}

pub enum CashFlowType {
    Dividend {issuer: String},
    Tax {issuer: String},
}

impl CashFlow {
    pub fn new(date: DateOptTime, amount: Cash, type_: CashFlowType) -> CashFlow {
        CashFlow {date, amount, type_}
    }
}