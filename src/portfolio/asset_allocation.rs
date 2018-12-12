use std::collections::{HashSet, HashMap};

use crate::brokers::BrokerInfo;
use crate::config::{Config, PortfolioConfig, AssetAllocationConfig};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::quotes::Quotes;
use crate::types::Decimal;
use crate::util;

use super::Assets;

pub struct Portfolio {
    pub name: String,
    pub broker: BrokerInfo,
    pub currency: String,

    pub min_trade_volume: Decimal,
    pub min_cash_assets: Decimal,

    pub assets: Vec<AssetAllocation>,
    pub current_cash_assets: Decimal,
    pub target_cash_assets: Decimal,
    pub commissions: Decimal,
    pub total_value: Decimal,
}

impl Portfolio {
    pub fn load(
        config: &Config, portfolio_config: &PortfolioConfig, assets: Assets,
        converter: &CurrencyConverter, quotes: &mut Quotes
    ) -> GenericResult<Portfolio> {
        let currency = match portfolio_config.currency.as_ref() {
            Some(currency) => currency,
            None => return Err!("The portfolio's currency is not specified in the config"),
        };

        let min_trade_volume = portfolio_config.min_trade_volume.unwrap_or(dec!(0));
        if min_trade_volume.is_sign_negative() {
            return Err!("Invalid minimum trade volume value")
        }

        let min_cash_assets = portfolio_config.min_cash_assets.unwrap_or(dec!(0));
        if min_cash_assets.is_sign_negative() {
            return Err!("Invalid minimum free cash assets value")
        }

        if portfolio_config.assets.is_empty() {
            return Err!("The portfolio has no asset allocation configuration");
        }

        for symbol in portfolio_config.get_stock_symbols() {
            quotes.batch(&symbol);
        }

        let cash_assets = assets.cash.total_assets(&currency, converter)?;

        let mut portfolio = Portfolio {
            name: portfolio_config.name.clone(),
            broker: BrokerInfo::get(config, portfolio_config.broker)?,
            currency: currency.clone(),

            min_trade_volume: min_trade_volume,
            min_cash_assets: min_cash_assets,

            assets: Vec::new(),
            current_cash_assets: cash_assets,
            target_cash_assets: cash_assets,
            commissions: dec!(0),
            total_value: cash_assets,
        };

        let mut stocks = assets.stocks;
        let mut symbols = HashSet::new();

        for assets_config in &portfolio_config.assets {
            let mut asset_allocation = AssetAllocation::load(
                assets_config, &currency, &mut symbols, &mut stocks, converter, quotes)?;

            asset_allocation.apply_restrictions(
                portfolio_config.restrict_buying, portfolio_config.restrict_selling);

            portfolio.total_value += asset_allocation.current_value;
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

pub enum Holding {
    Stock(StockHolding),
    Group(Vec<AssetAllocation>),
}

pub struct StockHolding {
    pub symbol: String,
    pub price: Decimal,
    pub currency_price: Cash,
    pub current_shares: u32,
    pub target_shares: u32,
}

pub struct AssetAllocation {
    pub name: String,

    pub expected_weight: Decimal,
    pub restrict_buying: Option<bool>,
    pub restrict_selling: Option<bool>,

    pub holding: Holding,
    pub current_value: Decimal,
    pub target_value: Decimal,

    pub min_value: Decimal,
    pub max_value: Option<Decimal>,

    pub buy_blocked: bool,
    pub sell_blocked: bool,
}

impl AssetAllocation {
    pub fn full_name(&self) -> String {
        match self.holding {
            Holding::Group(_) => self.name.clone(),
            Holding::Stock(ref holding) => format!("{} ({})", self.name, holding.symbol),
        }
    }

    fn load(
        config: &AssetAllocationConfig, currency: &str,
        symbols: &mut HashSet<String>, stocks: &mut HashMap<String, u32>,
        converter: &CurrencyConverter, quotes: &mut Quotes,
    ) -> GenericResult<AssetAllocation> {
        let (holding, current_value) = match (&config.symbol, &config.assets) {
            (Some(symbol), None) => {
                if !symbols.insert(symbol.clone()) {
                    return Err!("Invalid asset allocation configuration: Duplicated symbol: {}",
                        symbol);
                }

                let currency_price = quotes.get(symbol)?;
                let price = converter.convert_to(util::today(), currency_price, currency)?;

                let shares = stocks.remove(symbol).unwrap_or(0);
                let current_value = Decimal::from(shares) * price;

                let holding = StockHolding {
                    symbol: symbol.clone(),
                    price: price,
                    currency_price: currency_price,
                    current_shares: shares,
                    target_shares: shares,
                };

                (Holding::Stock(holding), current_value)
            },
            (None, Some(assets)) => {
                let mut holdings = Vec::new();
                let mut current_value = dec!(0);

                for asset in assets {
                    let holding = AssetAllocation::load(
                        asset, currency, symbols, stocks, converter, quotes)?;

                    current_value += holding.current_value;
                    holdings.push(holding);
                }

                check_weights(&config.name, &holdings)?;

                (Holding::Group(holdings), current_value)
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

            holding: holding,
            current_value: current_value,
            target_value: current_value,

            min_value: dec!(0),
            max_value: None,

            buy_blocked: false,
            sell_blocked: false,
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