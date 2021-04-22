use crate::currency::Cash;
use crate::time::Date;

// Provides detailed information about actual cash flows on account including reversal operations.
// Used to be able to calculate cash balance for specific point of time.
pub struct TechnicalCashFlow {
    pub date: Date,
    pub amount: Cash,
    pub type_: TechnicalCashFlowType,
}

pub enum TechnicalCashFlowType {
    Dividend {issuer: String},
    Tax {symbol: String},
}