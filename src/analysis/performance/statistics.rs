use std::collections::BTreeMap;

use log::warn;

use crate::brokers::Broker;
use crate::core::EmptyResult;
use crate::currency::{self, Cash};
use crate::localities::Country;
use crate::taxes::{LtoDeduction, NetLtoDeduction, TaxCalculator};
use crate::types::Decimal;

use super::types::{PerformanceAnalysisMethod, PortfolioPerformanceAnalysis};

pub struct PortfolioStatistics {
    country: Country,
    pub currencies: Vec<PortfolioCurrencyStatistics>,
    pub asset_groups: BTreeMap<String, AssetGroup>,
    pub lto: Option<LtoStatistics>,
}

pub struct LtoStatistics {
    pub applied: BTreeMap<i32, NetLtoDeduction>,
    pub projected: LtoDeduction,
}

impl PortfolioStatistics {
    pub fn new(country: Country) -> PortfolioStatistics {
        PortfolioStatistics {
            country: country.clone(),
            currencies: ["USD", "RUB"].iter().map(|&currency| (
                PortfolioCurrencyStatistics {
                    currency: currency.to_owned(),

                    assets: BTreeMap::new(),
                    brokers: BTreeMap::new(),

                    virtual_performance: None,
                    real_performance: None,
                    inflation_adjusted_performance: None,

                    projected_taxes: dec!(0),
                    projected_tax_deductions: dec!(0),
                    projected_commissions: dec!(0),
                }
            )).collect(),
            asset_groups: BTreeMap::new(),
            lto: None,
        }
    }

    pub fn print(&self, method: PerformanceAnalysisMethod) {
        let lto = self.lto.as_ref().unwrap();

        if method.tax_aware() {
            for (year, result) in &lto.applied {
                if !result.loss.is_zero() {
                    warn!("Long-term ownership tax deduction loss in {}: {}.",
                          year, self.country.cash(result.loss));
                }

                if !result.applied_above_limit.is_zero() {
                    warn!("Long-term ownership tax deductions applied in {} have exceeded the total limit by {}.",
                          year, self.country.cash(result.applied_above_limit));
                }
            }
        }

        for statistics in &self.currencies {
            statistics.performance(method).print(&format!(
                "Average rate of return from cash investments in {}", &statistics.currency));
        }

        if method.tax_aware() && !lto.projected.deduction.is_zero() {
            lto.projected.print("Projected LTO deduction")
        }
    }

    pub fn process<F>(&mut self, mut handler: F) -> EmptyResult
        where F: FnMut(&mut PortfolioCurrencyStatistics) -> EmptyResult
    {
        for statistics in &mut self.currencies {
            handler(statistics)?;
        }

        Ok(())
    }

    pub fn commit(self) -> Self {
        let asset_groups = self.asset_groups.into_iter().map(|(name, value)| {
            (name, value.commit())
        }).collect();

        PortfolioStatistics {
            country: self.country,
            currencies: self.currencies.into_iter().map(PortfolioCurrencyStatistics::commit).collect(),
            asset_groups,
            lto: self.lto,
        }
    }
}

pub struct PortfolioCurrencyStatistics {
    pub currency: String,

    // Use BTreeMap to get consistent metrics order
    pub assets: BTreeMap<String, BTreeMap<String, Asset>>,
    pub brokers: BTreeMap<Broker, Decimal>,

    pub virtual_performance: Option<PortfolioPerformanceAnalysis>,
    pub real_performance: Option<PortfolioPerformanceAnalysis>,
    pub inflation_adjusted_performance: Option<PortfolioPerformanceAnalysis>,

    pub projected_taxes: Decimal,
    pub projected_tax_deductions: Decimal,
    pub projected_commissions: Decimal,
}

impl PortfolioCurrencyStatistics {
    pub fn add_assets(&mut self, portfolio: &str, broker: Broker, instrument: &str, amount: Decimal, net_amount: Decimal) {
        let instrument = self.assets.entry(instrument.to_owned()).or_default();

        instrument.entry(portfolio.to_owned()).or_default().add(&Asset {
            value: amount,
            net_value: net_amount,
        });

        *self.brokers.entry(broker).or_default() += amount;
    }

    pub fn performance(&self, method: PerformanceAnalysisMethod) -> &PortfolioPerformanceAnalysis {
        match method {
            PerformanceAnalysisMethod::Virtual => &self.virtual_performance,
            PerformanceAnalysisMethod::Real => &self.real_performance,
            PerformanceAnalysisMethod::InflationAdjusted => &self.inflation_adjusted_performance,
        }.as_ref().unwrap()
    }

    pub fn set_performance(&mut self, method: PerformanceAnalysisMethod, performance: PortfolioPerformanceAnalysis) {
        let container = match method {
            PerformanceAnalysisMethod::Virtual => &mut self.virtual_performance,
            PerformanceAnalysisMethod::Real => &mut self.real_performance,
            PerformanceAnalysisMethod::InflationAdjusted => &mut self.inflation_adjusted_performance,
        };
        assert!(container.replace(performance).is_none());
    }

    fn commit(self) -> Self {
        let assets = self.assets.into_iter().map(|(instrument, portfolios)| {
            let portfolios = portfolios.into_iter().map(|(portfolio, asset)| {
                (portfolio, asset.commit())
            }).collect();

            (instrument, portfolios)
        }).collect();

        let brokers = self.brokers.into_iter().map(|(broker, value)| {
            (broker, currency::round(value))
        }).collect();

        PortfolioCurrencyStatistics {
            currency: self.currency,

            assets, brokers,

            virtual_performance: self.virtual_performance.map(PortfolioPerformanceAnalysis::commit),
            real_performance: self.real_performance.map(PortfolioPerformanceAnalysis::commit),
            inflation_adjusted_performance: self.inflation_adjusted_performance.map(PortfolioPerformanceAnalysis::commit),

            projected_taxes: currency::round(self.projected_taxes),
            projected_tax_deductions: currency::round(self.projected_tax_deductions),
            projected_commissions: currency::round(self.projected_commissions),
        }
    }
}

pub struct AssetGroup {
    pub taxes: TaxCalculator,
    pub net_value: Vec<Cash>,
}

impl AssetGroup {
    fn commit(self) -> Self {
        AssetGroup {
            taxes: self.taxes,
            net_value: self.net_value.into_iter().map(Cash::round).collect(),
        }
    }
}

#[derive(Default)]
pub struct Asset {
    pub value: Decimal,
    pub net_value: Decimal,
}

impl Asset {
    pub fn add(&mut self, other: &Asset) {
        self.value += other.value;
        self.net_value += other.net_value;
    }

    fn commit(self) -> Self {
        Asset {
            value: currency::round(self.value),
            net_value: currency::round(self.net_value),
        }
    }
}