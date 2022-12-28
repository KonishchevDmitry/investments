use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use validator::Validate;

use crate::analysis::config::{AssetGroupConfig, PerformanceMergingConfig};
use crate::core::EmptyResult;

#[derive(Deserialize, Default, Validate)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    #[validate]
    #[serde(default)]
    pub asset_groups: HashMap<String, AssetGroupConfig>,

    #[serde(default)]
    pub merge_performance: PerformanceMergingConfig,
}

impl MetricsConfig {
    pub fn validate(&self, portfolios: &HashSet<String>) -> EmptyResult {
        for (name, group) in &self.asset_groups {
            group.validate(portfolios).map_err(|e| format!("{:?} asset group: {}", name, e))?;
        }
        Ok(())
    }
}