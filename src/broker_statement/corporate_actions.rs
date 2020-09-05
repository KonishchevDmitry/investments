use crate::types::Date;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CorporateAction {
    StockSplit {
        date: Date,
        symbol: String,
        divisor: u32,
    }
}