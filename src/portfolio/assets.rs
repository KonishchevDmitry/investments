use std::collections::{HashSet, HashMap};
use std::str::FromStr;

use diesel::{self, prelude::*};

use config::PortfolioConfig;
use core::{EmptyResult, GenericError, GenericResult};
use currency::{Cash, MultiCurrencyCashAccount};
use db::{self, schema::{AssetType, assets}, models};
use types::Decimal;

#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
pub struct Assets {
    pub cash: MultiCurrencyCashAccount,
    pub stocks: HashMap<String, u32>,
}

impl Assets {
    pub fn new(cash: MultiCurrencyCashAccount, stocks: HashMap<String, u32>) -> Assets {
        Assets {
            cash: cash,
            stocks: stocks,
        }
    }

    pub fn load(database: db::Connection, portfolio: &str) -> GenericResult<Assets> {
        let assets = assets::table.filter(assets::portfolio.eq(portfolio))
            .load::<models::Asset>(&*database)?;

        let mut cash = MultiCurrencyCashAccount::new();
        let mut stocks = HashMap::new();

        for asset in assets {
            match asset.asset_type {
                AssetType::Cash => {
                    let amount = Decimal::from_str(&asset.quantity).map_err(|_| format!(
                        "Got an invalid cash amount from the database: {:?}", asset.quantity))?;

                    cash.deposit(Cash::new(&asset.symbol, amount));
                },

                AssetType::Stock => {
                    let quantity: u32 = asset.quantity.parse().map_err(|_| format!(
                        "Got an invalid stock quantity from the database: {}", asset.quantity))?;

                    if stocks.insert(asset.symbol.clone(), quantity).is_some() {
                        return Err!("Got a duplicated {} stock from the database", asset.symbol);
                    }
                },
            };
        }

        Ok(Assets::new(cash, stocks))
    }

    pub fn validate(&self, portfolio: &PortfolioConfig) -> EmptyResult {
        let portfolio_symbols = portfolio.get_stock_symbols();

        let mut assets_symbols = HashSet::new();
        assets_symbols.extend(self.stocks.keys().map(|symbol| symbol.to_owned()));

        let mut missing_symbols: Vec<String> = assets_symbols.difference(&portfolio_symbols)
            .map(|symbol| symbol.to_owned()).collect();
        missing_symbols.sort();

        if !missing_symbols.is_empty() {
            return Err!(
                "The portfolio contains stocks which are missing in asset allocation configuration: {}",
                missing_symbols.join(", "));
        }

        Ok(())
    }

    pub fn save(&self, database: db::Connection, portfolio: &str) -> EmptyResult {
        database.transaction::<_, GenericError, _>(|| {
            diesel::delete(assets::table.filter(assets::portfolio.eq(portfolio)))
                .execute(&*database)?;

            let mut assets = Vec::new();

            for (currency, amount) in self.cash.iter() {
                assets.push(models::Asset {
                    portfolio: portfolio.to_owned(),
                    asset_type: AssetType::Cash,
                    symbol: currency.to_string(),
                    quantity: amount.to_string(),
                })
            }

            for (symbol, quantity) in &self.stocks {
                assets.push(models::Asset {
                    portfolio: portfolio.to_owned(),
                    asset_type: AssetType::Stock,
                    symbol: symbol.to_owned(),
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

        let second_assets = Assets::new(MultiCurrencyCashAccount::new(), HashMap::new());
        assert_eq!(Assets::load(connection.clone(), "second").unwrap(), second_assets);

        let third_assets = {
            let mut cash = MultiCurrencyCashAccount::new();
            cash.deposit(Cash::new("RUB", dec!(10)));
            cash.deposit(Cash::new("EUR", dec!(20)));

            let mut stocks = HashMap::new();
            stocks.insert(s!("DDD"), 100);
            stocks.insert(s!("BBB"), 200);
            stocks.insert(s!("EEE"), 300);

            Assets::new(cash, stocks)
        };

        first_assets.save(connection.clone(), "first").unwrap();
        second_assets.save(connection.clone(), "second").unwrap();
        third_assets.save(connection.clone(), "third").unwrap();

        assert_eq!(Assets::load(connection.clone(), "first").unwrap(), first_assets);
        assert_eq!(Assets::load(connection.clone(), "second").unwrap(), second_assets);
        assert_eq!(Assets::load(connection.clone(), "third").unwrap(), third_assets);

        third_assets.save(connection.clone(), "first").unwrap();
        third_assets.save(connection.clone(), "second").unwrap();
        second_assets.save(connection.clone(), "third").unwrap();

        assert_eq!(Assets::load(connection.clone(), "first").unwrap(), third_assets);
        assert_eq!(Assets::load(connection.clone(), "second").unwrap(), third_assets);
        assert_eq!(Assets::load(connection.clone(), "third").unwrap(), second_assets);
    }
}