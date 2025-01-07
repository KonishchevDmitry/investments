use log::warn;

use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::localities::Jurisdiction;
use crate::time::Date;
use crate::types::Decimal;

use super::BrokerStatement;
use super::trades::StockBuy;

pub struct CashGrant {
    pub date: Date,
    pub amount: Cash,
    pub description: String,
}

impl CashGrant {
    pub fn new(date: Date, amount: Cash, description: &str) -> CashGrant {
        CashGrant{
            date, amount,
            description: description.to_owned(),
        }
    }
}

pub struct StockGrant {
    pub date: Date,
    pub symbol: String,
    pub quantity: Decimal,
}

impl StockGrant {
    pub fn new(date: Date, symbol: &str, quantity: Decimal) -> StockGrant {
        StockGrant{
            date,
            symbol: symbol.to_owned(),
            quantity: quantity,
        }
    }
}

pub fn process_grants(statement: &mut BrokerStatement, strict: bool) -> EmptyResult {
    // For now I saw only grants from Sber which have 100% tax deduction, so we don't process any taxation for them
    if !statement.cash_grants.is_empty() && strict && statement.broker.type_.jurisdiction() != Jurisdiction::Russia {
        warn!("The statement contains cash grants which is not supported yet (won't be taxed).");
    }

    if !statement.stock_grants.is_empty() {
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
    }

    Ok(())
}