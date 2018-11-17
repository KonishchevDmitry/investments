use std::collections::{HashSet, HashMap};

use ansi_term::Style;

use config::{PortfolioConfig, AssetAllocationConfig};
use core::{EmptyResult, GenericResult};
use formatting;
use types::Decimal;

use super::Assets;

pub struct Portfolio {
    name: String,
    assets: Vec<AssetAllocation>,
}

impl Portfolio {
    pub fn load(config: &PortfolioConfig, assets: &Assets) -> GenericResult<Portfolio> {
        if config.assets.is_empty() {
            return Err!("The portfolio has no asset allocation configuration");
        }

        let mut portfolio = Portfolio {
            name: config.name.clone(),
            assets: Vec::new(),
        };

        let mut stocks = assets.stocks.clone();
        let mut symbols = HashSet::new();

        for assets_config in &config.assets {
            portfolio.assets.push(
                AssetAllocation::load(assets_config, &mut symbols, &mut stocks)?);
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

    // FIXME: flat mode
    pub fn print(&self) {
        for assets in &self.assets {
            assets.print(0);
        }
    }
}

pub struct AssetAllocation {
    name: String,
    symbol: Option<String>,
    weight: Decimal,
    assets: Vec<AssetAllocation>, // FIXME: Option?
}

impl AssetAllocation {
    fn load(
        config: &AssetAllocationConfig, symbols: &mut HashSet<String>,
        stocks: &mut HashMap<String, u32>
    ) -> GenericResult<AssetAllocation> {
        let mut asset_allocation = AssetAllocation {
            name: config.name.clone(),
            symbol: None,
            weight: config.weight,
            assets: Vec::new(),
        };

        match (&config.symbol, &config.assets) {
            (Some(symbol), None) => {
                if !symbols.insert(symbol.clone()) {
                    return Err!("Invalid asset allocation configuration: Duplicated symbol: {}",
                        symbol);
                }

                if let Some(shares) = stocks.remove(symbol) {
                    // FIXME: HERE
                }

                asset_allocation.symbol = Some(symbol.clone());
            },
            (None, Some(assets)) => {
                for asset in assets {
                    asset_allocation.assets.push(
                        AssetAllocation::load(asset, symbols, stocks)?);
                }

                check_weights(&asset_allocation.name, &asset_allocation.assets)?;
            },
            _ => return Err!(
               "Invalid {:?} assets configuration: either symbol or assets must be specified",
               config.name),
        };

        Ok(asset_allocation)
    }

    pub fn print(&self, depth: usize) {
        for assets in &self.assets {
            let suffix = if assets.assets.is_empty() {
                ""
            } else {
                ":"
            };

            println!("{bullet:>depth$} {name} - {weight}{suffix}",
                     bullet='*', name=Style::new().bold().paint(&assets.name),
                     weight=formatting::format_weight(assets.weight),
                     suffix=suffix, depth=depth * 2 + 1);

            assets.print(depth + 1);
        }
    }
}

fn check_weights(name: &str, assets: &Vec<AssetAllocation>) -> EmptyResult {
    let mut weight = dec!(0);

    for asset in assets {
        weight += asset.weight;
    }

    if weight != dec!(1) {
        return Err!("{:?} assets have unbalanced weights: {}% total",
            name, (weight * dec!(100)).normalize());
    }

    Ok(())
}