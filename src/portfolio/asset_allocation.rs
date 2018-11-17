use std::collections::{HashSet, HashMap};

use ansi_term::Style;

use config::{PortfolioConfig, AssetAllocationConfig};
use core::{EmptyResult, GenericResult};
use formatting;
use types::Decimal;

use super::Assets;

pub struct AssetAllocation {
    name: String,
    symbol: Option<String>,
    weight: Decimal,
    assets: Vec<AssetAllocation>, // FIXME: Option?
}

impl AssetAllocation {
    pub fn load(portfolio: &PortfolioConfig, assets: &Assets) -> GenericResult<AssetAllocation> {
        if portfolio.assets.is_empty() {
            return Err!("The portfolio has no asset allocation configuration");
        }

        let mut stocks = assets.stocks.clone();
        let mut symbols = HashSet::new();

        let mut asset_allocation = AssetAllocation {
            name: portfolio.name.clone(), // FIXME
            symbol: None,
            weight: dec!(1),
            assets: Vec::new(),
        };

        for config in &portfolio.assets {
            asset_allocation.assets.push(
                AssetAllocation::load_inner(config, &mut symbols, &mut stocks)?);
        }
        asset_allocation.check_weights()?;

        if !stocks.is_empty() {
            let mut missing_symbols: Vec<String> = stocks.keys()
                .map(|symbol| symbol.to_owned()).collect();

            missing_symbols.sort();

            return Err!(
                "The portfolio contains stocks that are missing in asset allocation configuration: {}",
                missing_symbols.join(", "));
        }

        Ok(asset_allocation)
    }

    fn load_inner(
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
                        AssetAllocation::load_inner(asset, symbols, stocks)?);
                }
                asset_allocation.check_weights()?;
            },
            _ => return Err!(
               "Invalid {:?} assets configuration: either symbol or assets must be specified",
               config.name),
        };

        Ok(asset_allocation)
    }

    fn check_weights(&self) -> EmptyResult {
        let mut weight = dec!(0);

        for assets in &self.assets {
            weight += assets.weight;
        }

        if weight != dec!(1) {
            return Err!("{:?} assets have unbalanced weights: {}% total",
                self.name, (weight * dec!(100)).normalize());
        }

        Ok(())
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