use std::collections::HashMap;

use diesel::{self, prelude::*};

use core::{EmptyResult, GenericError, GenericResult};
use currency::{Cash, MultiCurrencyCashAccount};
use db::{self, schema::{AssetType, assets}, models::NewAsset};

pub struct Assets {
    cash: MultiCurrencyCashAccount,
    stocks: HashMap<String, u32>,
}

impl Assets {
    pub fn new(cash: MultiCurrencyCashAccount, stocks: HashMap<String, u32>) -> Assets {
        Assets {
            cash: cash,
            stocks: stocks,
        }
    }

    pub fn save(&self, database: db::Connection, portfolio: &str) -> EmptyResult {
        database.transaction::<_, GenericError, _>(|| {
            diesel::delete(assets::table.filter(assets::portfolio.eq(portfolio)))
                .execute(&*database)?;

            let mut assets = Vec::new();

            for (currency, amount) in self.cash.iter() {
                assets.push(NewAsset {
                    portfolio: portfolio,
                    asset_type: AssetType::Cash,
                    symbol: currency,
                    quantity: amount.to_string(),
                })
            }

            for (symbol, quantity) in &self.stocks {
                assets.push(NewAsset {
                    portfolio: portfolio,
                    asset_type: AssetType::Stock,
                    symbol: symbol,
                    quantity: quantity.to_string(),
                })
            }

            diesel::insert_into(assets::table)
                .values(&assets)
                .execute(&*database)?;

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load() {
        let (_database, connection) = db::new_temporary();

        let first_assets = {
            let mut cash = MultiCurrencyCashAccount::new();
            cash.deposit(Cash::new("RUB", dec!(100)));
            cash.deposit(Cash::new("USD", dec!(200)));

            let mut stocks = HashMap::new();
            stocks.insert(s!("AAA"), 10);
            stocks.insert(s!("BBB"), 20);
            stocks.insert(s!("CCC"), 30);

            Assets::new(cash, stocks)
        };

        let second_assets = {
            let mut cash = MultiCurrencyCashAccount::new();
            cash.deposit(Cash::new("RUB", dec!(10)));
            cash.deposit(Cash::new("USD", dec!(20)));

            let mut stocks = HashMap::new();
            stocks.insert(s!("DDD"), 100);
            stocks.insert(s!("BBB"), 200);
            stocks.insert(s!("EEE"), 300);

            Assets::new(cash, stocks)
        };

        first_assets.save(connection.clone(), "first").unwrap();
        first_assets.save(connection.clone(), "second").unwrap();
    }
}