use crate::types::Date;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CorporateAction {
    pub date: Date,
    pub symbol: String,
    pub action: CorporateActionType,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CorporateActionType {
    StockSplit(u32),
}