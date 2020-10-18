use std::collections::{HashSet, HashMap};

use num_traits::Zero;

use crate::brokers::BrokerInfo;
use crate::config::{Config, PortfolioConfig, AssetAllocationConfig};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::quotes::Quotes;
use crate::types::{Decimal, TradeType};
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
    pub current_net_value: Decimal,

    pub target_cash_assets: Decimal,
    pub target_net_value: Decimal,
    pub commissions: Decimal,
}

impl Portfolio {
    pub fn load(
        config: &Config, portfolio_config: &PortfolioConfig, assets: Assets,
        converter: &CurrencyConverter, quotes: &Quotes
    ) -> GenericResult<Portfolio> {
        let currency = portfolio_config.currency()?;
        let broker = portfolio_config.broker.get_info(config, portfolio_config.plan.as_ref())?;

        let min_trade_volume = portfolio_config.min_trade_volume.unwrap_or_else(|| dec!(0));
        if min_trade_volume.is_sign_negative() {
            return Err!("Invalid minimum trade volume value")
        }

        let min_cash_assets = portfolio_config.min_cash_assets.unwrap_or_else(|| dec!(0));
        if min_cash_assets.is_sign_negative() {
            return Err!("Invalid minimum free cash assets value")
        }

        if portfolio_config.assets.is_empty() {
            return Err!("The portfolio has no asset allocation configuration");
        }

        for symbol in portfolio_config.get_stock_symbols() {
            quotes.batch(&symbol)?;
        }

        let cash_assets = assets.cash.total_assets_real_time(&currency, converter)?;
        let mut net_value = cash_assets;

        let mut stocks = assets.stocks;
        let mut symbols = HashSet::new();
        let mut assets_allocation = Vec::new();

        for assets_config in &portfolio_config.assets {
            let mut asset_allocation = AssetAllocation::load(
                &broker, assets_config, &currency, &mut symbols, &mut stocks,
                converter, quotes)?;

            asset_allocation.apply_restrictions(
                portfolio_config.restrict_buying, portfolio_config.restrict_selling);

            net_value += asset_allocation.current_value;
            assets_allocation.push(asset_allocation);
        }

        let portfolio = Portfolio {
            name: portfolio_config.name.clone(),
            broker: broker,
            currency: currency.to_owned(),

            min_trade_volume: min_trade_volume,
            min_cash_assets: min_cash_assets,

            assets: assets_allocation,
            current_cash_assets: cash_assets,
            current_net_value: net_value,

            target_cash_assets: cash_assets,
            target_net_value: net_value,
            commissions: dec!(0),
        };
        check_weights(&portfolio.name, &portfolio.assets)?;

        if !stocks.is_empty() {
            let mut missing_symbols: Vec<String> = stocks.keys().cloned().collect();
            missing_symbols.sort_unstable();

            return Err!(
                    "The portfolio contains stocks that are missing in asset allocation configuration: {}",
                    missing_symbols.join(", "));
        }

        Ok(portfolio)
    }

    pub fn change_commission(&mut self, commission: Decimal) {
        // The commission may be positive in case of withdrawal or negative in case of reverting of
        // previously withdrawn commission.

        self.commissions += commission;
        self.target_net_value -= commission;
        self.target_cash_assets -= commission;
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
    pub current_shares: Decimal,
    pub target_shares: Decimal,
    pub fractional_shares_trading: bool,
}

impl StockHolding {
    pub fn trade_granularity(&self) -> Decimal {
        self.trade_precision_volume(self.trade_precision())
    }

    pub fn iterative_trading_granularity(&self, trade_type: TradeType) -> Decimal {
        let mut precision = self.trade_precision();
        let mut volume = self.trade_precision_volume(precision);

        while precision > 0 && volume < dec!(1) {
            precision -= 1;
            volume = self.trade_precision_volume(precision);
        }

        if matches!(trade_type,
            TradeType::Sell
            if self.fractional_shares_trading &&
                volume > self.target_shares && !self.target_shares.is_zero()
        ) {
            volume = self.price * self.target_shares;
        }

        volume
    }

    fn trade_precision(&self) -> u32 {
        if self.fractional_shares_trading {
            util::decimal_precision(self.current_shares)
        } else {
            0
        }
    }

    fn trade_precision_volume(&self, precision: u32) -> Decimal {
        self.price * Decimal::new(1, precision)
    }
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
    fn load(
        broker: &BrokerInfo, config: &AssetAllocationConfig, currency: &str,
        symbols: &mut HashSet<String>, stocks: &mut HashMap<String, Decimal>,
        converter: &CurrencyConverter, quotes: &Quotes,
    ) -> GenericResult<AssetAllocation> {
        let (holding, current_value) = match (&config.symbol, &config.assets) {
            (Some(symbol), None) => {
                if !symbols.insert(symbol.clone()) {
                    return Err!("Invalid asset allocation configuration: Duplicated symbol: {}",
                        symbol);
                }

                let currency_price = quotes.get(symbol)?;
                let price = converter.real_time_convert_to(currency_price, currency)?;
                let shares = stocks.remove(symbol).unwrap_or_else(|| dec!(0));
                let current_value = shares * price;

                let holding = StockHolding {
                    symbol: symbol.clone(),
                    price: price,
                    currency_price: currency_price,
                    current_shares: shares,
                    target_shares: shares,
                    fractional_shares_trading: broker.fractional_shares_trading,
                };

                (Holding::Stock(holding), current_value)
            },
            (None, Some(assets)) => {
                let mut holdings = Vec::new();
                let mut current_value = dec!(0);

                for asset in assets {
                    let holding = AssetAllocation::load(
                        broker, asset, currency, symbols, stocks, converter, quotes)?;

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

    pub fn full_name(&self) -> String {
        match self.holding {
            Holding::Group(_) => self.name.clone(),
            Holding::Stock(ref holding) => format!("{} ({})", self.name, holding.symbol),
        }
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

    // FIXME(konishchev): Iteratively decrease trade granularity to fix performance issues
    pub fn iterative_trading_granularity(&self, trade_type: TradeType) -> Decimal {
        match self.holding {
            Holding::Stock(ref holding) => holding.iterative_trading_granularity(trade_type),
            Holding::Group(ref holdings) => {
                let mut min_granularity = None;

                for holding in holdings {
                    let granularity = holding.iterative_trading_granularity(trade_type);

                    min_granularity = Some(match min_granularity {
                        Some(min_granularity) if min_granularity <= granularity => min_granularity,
                        _ => granularity,
                    });
                }

                min_granularity.unwrap()
            },
        }
    }
}

fn check_weights(name: &str, assets: &[AssetAllocation]) -> EmptyResult {
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