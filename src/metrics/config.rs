use std::collections::{BTreeSet, HashMap, HashSet};

use serde::Deserialize;
use validator::Validate;

use crate::analysis::config::{AssetGroupConfig, PerformanceMergingConfig};
use crate::core::EmptyResult;

#[derive(Deserialize, Default, Validate)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    #[serde(default)]
    #[validate(custom = "crate::forex::validate_currency_pair_list")]
    pub currency_rates: BTreeSet<String>,

    #[validate]
    #[serde(default)]
    pub asset_groups: HashMap<String, AssetGroupConfig>,

    #[serde(default)]
    pub merge_performance: PerformanceMergingConfig,
}

impl MetricsConfig {
    pub fn validate_inner(&self, portfolios: &HashSet<String>) -> EmptyResult {
        for (name, group) in &self.asset_groups {
            group.validate_inner(portfolios).map_err(|e| format!(
                "{:?} asset group: {}", name, e))?;
        }
        Ok(())
    }
}