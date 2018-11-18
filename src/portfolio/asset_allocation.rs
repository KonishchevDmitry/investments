use std::collections::{HashSet, HashMap};

use config::{PortfolioConfig, AssetAllocationConfig};
use core::{EmptyResult, GenericResult};
use currency::converter::CurrencyConverter;
use quotes::Quotes;
use types::Decimal;
use util;

use super::Assets;

pub struct Portfolio {
    pub name: String,
    pub currency: String,
    pub assets: Vec<AssetAllocation>,

    pub total_value: Decimal,
    pub free_assets: Decimal,
}

impl Portfolio {
    pub fn load(
        config: &PortfolioConfig, assets: Assets,
        converter: &CurrencyConverter, quotes: &mut Quotes
    ) -> GenericResult<Portfolio> {
        let currency = match config.currency.as_ref() {
            Some(currency) => currency,
            None => return Err!("The portfolio's currency is not specified in the config"),
        };

        if config.assets.is_empty() {
            return Err!("The portfolio has no asset allocation configuration");
        }

        for symbol in config.get_stock_symbols() {
            quotes.batch(&symbol);
        }

        let free_assets = assets.cash.total_assets(&currency, converter)?;

        let mut portfolio = Portfolio {
            name: config.name.clone(),
            currency: currency.clone(),
            assets: Vec::new(),
            total_value: free_assets,
            free_assets: free_assets,
        };

        let mut stocks = assets.stocks;
        let mut symbols = HashSet::new();

        for assets_config in &config.assets {
            let mut asset_allocation = AssetAllocation::load(
                assets_config, &currency, &mut symbols, &mut stocks, converter, quotes)?;
            asset_allocation.apply_restrictions(config.restrict_buying, config.restrict_selling);

            portfolio.total_value += asset_allocation.value;
            portfolio.assets.push(asset_allocation);
        }
        check_weights(&portfolio.name, &portfolio.assets)?;

        if !stocks.is_empty() {
            let mut missing_symbols: Vec<String> = stocks.keys()
                .map(|symbol| symbol.to_owned()).collect();

            missing_symbols.sort();

            return Err!(
                    "The portfolio contains stocks that are missing in asset allocation configuration: {}",
                    missing_symbols.join(", "));
        }

        Ok(portfolio)
    }
}

// FIXME: name
pub enum Holding {
    Stock(StockHolding),
    Group(Vec<AssetAllocation>),
}

impl Holding {
    fn value(&self) -> Decimal {
        match self {
            Holding::Stock(holding) => {
                Decimal::from(holding.shares) * holding.price
            },
            Holding::Group(assets) => {
                let mut value = dec!(0);

                for asset in assets {
                    value += asset.value;
                }

                value
            },
        }
    }
}

// FIXME: name
pub struct StockHolding {
    pub symbol: String,
    pub shares: u32,
    pub price: Decimal,
}

pub struct AssetAllocation {
    pub name: String,

    pub expected_weight: Decimal,
    pub restrict_buying: Option<bool>,
    pub restrict_selling: Option<bool>,

    pub holding: Holding,
    pub value: Decimal,
}

impl AssetAllocation {
    fn load(
        config: &AssetAllocationConfig, currency: &str,
        symbols: &mut HashSet<String>, stocks: &mut HashMap<String, u32>,
        converter: &CurrencyConverter, quotes: &mut Quotes,
    ) -> GenericResult<AssetAllocation> {
        let holding = match (&config.symbol, &config.assets) {
            (Some(symbol), None) => {
                if !symbols.insert(symbol.clone()) {
                    return Err!("Invalid asset allocation configuration: Duplicated symbol: {}",
                        symbol);
                }

                let price = converter.convert_to(
                    util::today(), quotes.get(symbol)?, currency)?;

                Holding::Stock(StockHolding {
                    symbol: symbol.clone(),
                    shares: stocks.remove(symbol).unwrap_or(0),
                    price: price,
                })
            },
            (None, Some(assets)) => {
                let mut holdings = Vec::new();

                for asset in assets {
                    holdings.push(AssetAllocation::load(
                        asset, currency, symbols, stocks, converter, quotes)?);
                }

                check_weights(&config.name, &holdings)?;

                Holding::Group(holdings)
            },
            _ => return Err!(
               "Invalid {:?} assets configuration: either symbol or assets must be specified",
               config.name),
        };

        let mut asset_allocation = AssetAllocation {
            name: config.name.clone(),

            expected_weight: config.weight,
            restrict_buying: None,
            restrict_selling: None,

            value: holding.value(),
            holding: holding,
        };

        asset_allocation.apply_restrictions(config.restrict_buying, config.restrict_selling);

        Ok(asset_allocation)
    }

    fn apply_restrictions(&mut self, restrict_buying: Option<bool>, restrict_selling: Option<bool>) {
        if let Some(restrict) = restrict_buying {
            self.apply_buying_restriction(restrict);
        }

        if let Some(restrict) = restrict_selling {
            self.apply_selling_restriction(restrict);
        }
    }

    fn apply_buying_restriction(&mut self, restrict: bool) {
        if self.restrict_buying.is_some() {
            return
        }

        self.restrict_buying = Some(restrict);

        if let Holding::Group(ref mut assets) = self.holding {
            for asset in assets {
                asset.apply_buying_restriction(restrict);
            }
        }
    }

    fn apply_selling_restriction(&mut self, restrict: bool) {
        if self.restrict_selling.is_some() {
            return
        }

        self.restrict_selling = Some(restrict);

        if let Holding::Group(ref mut assets) = self.holding {
            for asset in assets {
                asset.apply_selling_restriction(restrict);
            }
        }
    }
}

fn check_weights(name: &str, assets: &Vec<AssetAllocation>) -> EmptyResult {
    let mut weight = dec!(0);

    for asset in assets {
        weight += asset.expected_weight;
    }

    if weight != dec!(1) {
        return Err!("{:?} assets have unbalanced weights: {}% total",
            name, (weight * dec!(100)).normalize());
    }

    Ok(())
}