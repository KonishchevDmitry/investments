use ansi_term::Style;

use config::{PortfolioConfig, AssetAllocationConfig};
use core::{EmptyResult, GenericResult};
use formatting;
use types::Decimal;

pub struct AssetAllocation {
    name: String,
    symbol: Option<String>,
    weight: Decimal,
    assets: Vec<AssetAllocation>, // FIXME: Option?
}

impl AssetAllocation {
    pub fn parse(portfolio: &PortfolioConfig) -> GenericResult<AssetAllocation> {
        if portfolio.assets.is_empty() {
            return Err!("The portfolio has no asset allocation configuration");
        }

        let mut assets = AssetAllocation {
            name: portfolio.name.clone(), // FIXME
            symbol: None,
            weight: dec!(1),
            assets: Vec::new(),
        };

        for assets_config in &portfolio.assets {
            assets.assets.push(AssetAllocation::from_config(assets_config)?);
        }
        assets.check_weights()?;

        Ok(assets)
    }

    fn from_config(config: &AssetAllocationConfig) -> GenericResult<AssetAllocation> {
        let mut assets = AssetAllocation {
            name: config.name.clone(),
            symbol: None,
            weight: config.weight,
            assets: Vec::new(),
        };

        match (&config.symbol, &config.assets) {
            (Some(symbol), None) => {
                assets.symbol = Some(symbol.clone());
            },
            (None, Some(assets_configs)) => {
                for assets_config in assets_configs {
                    assets.assets.push(AssetAllocation::from_config(assets_config)?);
                }
                assets.check_weights()?;
            },
            _ => return Err!(
               "Invalid {:?} assets configuration: either symbol or assets must be specified",
               config.name),
        };

        Ok(assets)
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