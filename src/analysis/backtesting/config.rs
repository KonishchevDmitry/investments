use std::collections::{BTreeMap, HashSet};

use serde::Deserialize;
use serde::de::{Deserializer, Error};
use validator::Validate;

use crate::core::EmptyResult;
use crate::exchanges::Exchange;
use crate::metrics;
use crate::time::{self, Date};

#[derive(Default, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct BacktestingConfig {
    #[serde(default)]
    #[validate(nested)]
    pub benchmarks: Vec<BenchmarkConfig>,
    pub deposit_benchmarks: Option<bool>,
}

impl BacktestingConfig {
    pub fn validate_inner(&self) -> EmptyResult {
        let mut ids = HashSet::new();

        for benchmark in &self.benchmarks {
            if benchmark.name == metrics::PORTFOLIO_INSTRUMENT {
                return Err!("Invalid benchmark name: {:?}", benchmark.name);
            }

            if !ids.insert((benchmark.name.clone(), benchmark.provider.clone())) {
                if let Some(provider) = benchmark.provider.as_ref() {
                    return Err!("Duplicated benchmark: {} / {}", benchmark.name, provider);
                } else {
                    return Err!("Duplicated benchmark: {}", benchmark.name);
                }
            }
        }

        Ok(())
    }
}

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkConfig {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(length(min = 1))]
    pub provider: Option<String>,

    #[validate(length(min = 1))]
    pub symbol: String,
    pub exchange: Exchange,
    #[serde(default)]
    pub aliases: Vec<String>,

    #[serde(default, deserialize_with = "deserialize_transitions")]
    pub transitions: BTreeMap<Date, InstrumentTransition>,
}

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct InstrumentTransition {
    #[validate(length(min = 1))]
    pub symbol: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub exchange: Option<Exchange>,
    pub transition_type: Option<TransitionType>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all="kebab-case")]
pub enum TransitionType {
    Convert,
    Rename,
}

fn deserialize_transitions<'de, D>(deserializer: D) -> Result<BTreeMap<Date, InstrumentTransition>, D::Error>
    where D: Deserializer<'de>
{
    let deserialized: BTreeMap<String, InstrumentTransition> = Deserialize::deserialize(deserializer)?;
    let mut transitions = BTreeMap::new();

    for (date, transition) in deserialized {
        let date = time::parse_user_date(&date).map_err(D::Error::custom)?;
        transitions.insert(date, transition);
    }

    Ok(transitions)
}