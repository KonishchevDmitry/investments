use serde::Deserialize;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::Decimal;

#[derive(Deserialize)]
pub struct CashAssets {
    #[serde(rename = "item")]
    cash_assets: Vec<CashAssetsItem>,
}

#[derive(Deserialize)]
struct CashAssetsItem {
    #[serde(rename = "currencycode")]
    currency: String,

    #[serde(rename = "amountin")]
    start_amount: Decimal,

    #[serde(rename = "amountplaneout")]
    end_amount: Decimal,
}

impl CashAssets {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> GenericResult<bool> {
        let mut has_starting_assets = false;

        for cash_assets in &self.cash_assets {
            has_starting_assets |= !cash_assets.start_amount.is_zero();

            let current_amount = Cash::new(&cash_assets.currency, cash_assets.end_amount);
            statement.assets.cash.as_mut().unwrap().deposit(current_amount);
        }

        Ok(has_starting_assets)
    }
}