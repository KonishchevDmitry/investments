use std::collections::{BTreeSet, HashMap, HashSet};

use serde::Deserialize;
use validator::Validate;

use crate::analysis::performance::config::{AssetGroupConfig, PerformanceMergingConfig};
use crate::core::EmptyResult;
use crate::metrics;

use super::backfilling::BackfillingConfig;

#[derive(Deserialize, Default, Validate)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    #[serde(default)]
    #[validate(custom(function = "crate::forex::validate_currency_pair_list"))]
    pub currency_rates: BTreeSet<String>,

    #[validate(nested)]
    #[serde(default)]
    pub asset_groups: HashMap<String, AssetGroupConfig>,

    #[serde(default)]
    pub merge_performance: PerformanceMergingConfig,

    #[validate(nested)]
    pub backfilling: Option<BackfillingConfig>,
}

impl MetricsConfig {
    pub fn validate_inner(&self, portfolios: &HashSet<String>) -> EmptyResult {
        for (name, group) in &self.asset_groups {
            if name == metrics::PORTFOLIO_INSTRUMENT {
                return Err!("Invalid asset group name: {name:?}")
            }

            group.validate_inner(portfolios).map_err(|e| format!(
                "{:?} asset group: {}", name, e))?;
        }
        Ok(())
    }
}