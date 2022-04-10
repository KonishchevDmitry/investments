use log::warn;

use crate::core::EmptyResult;
use crate::time::Date;
use crate::types::Decimal;

use super::BrokerStatement;
use super::trades::StockBuy;

pub struct StockGrant {
    pub date: Date,
    pub symbol: String,
    pub quantity: Decimal,
}

pub fn process_grants(statement: &mut BrokerStatement, strict: bool) -> EmptyResult {
    if statement.stock_grants.is_empty() {
        return Ok(());
    }

    if strict {
        warn!(concat!(
            "The statement contains stock grants which should be declared as material gain, but ",
            "the program doesn't support this yet and will consider them as a stock buy at zero price."
        ));
    }

    for grant in &statement.stock_grants {
        statement.stock_buys.push(StockBuy::new_grant(grant.date, &grant.symbol, grant.quantity));
    }
    statement.sort_and_validate_stock_buys()?;

    Ok(())
}