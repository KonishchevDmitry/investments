use std::collections::{BTreeSet, HashMap, HashSet};

use serde::Deserialize;
use serde::de::{Deserializer, Error};
use validator::Validate;

use crate::core::EmptyResult;

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct AssetGroupConfig {
    #[validate(length(min = 1))]
    pub instruments: HashSet<String>,

    #[validate(length(min = 1))]
    #[validate(custom = "crate::currency::validate_currency_list")]
    pub currencies: BTreeSet<String>,

    #[serde(default)]
    #[validate(length(min = 1))]
    pub portfolios: Option<HashSet<String>>,
}

impl AssetGroupConfig {
    pub fn validate_inner(&self, portfolios: &HashSet<String>) -> EmptyResult {
        if let Some(names) = self.portfolios.as_ref() {
            if let Some(name) = names.difference(portfolios).next() {
                return Err!("Invalid portfolio name: {:?}", name)
            }
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct PerformanceMergingConfig {
    mapping: HashMap<String, HashSet<String>>,
    reverse: HashMap<String, String>,
}

impl PerformanceMergingConfig {
    pub fn add(&mut self, other: &PerformanceMergingConfig) -> EmptyResult {
        Ok(self.add_mapping(other.mapping.clone()).map_err(|e| format!(
            "Invalid performance merging configuration: {}", e))?)
    }

    pub fn map<'a, 'b: 'a>(&'a self, symbol: &'b str) -> &'a str {
        self.reverse.get(symbol).map(String::as_str).unwrap_or(symbol)
    }

    fn add_mapping(&mut self, other: HashMap<String, HashSet<String>>) -> EmptyResult {
        let mut mapping = other;
        let mut reverse = HashMap::new();

        for (master_symbol, slave_symbols) in &mapping {
            for slave_symbol in slave_symbols {
                if mapping.get(slave_symbol).is_some() {
                    return Err!("Cycle mapping on {:?} symbol", slave_symbol);
                }
                if reverse.insert(slave_symbol.clone(), master_symbol.clone()).is_some() {
                    return Err!("Duplicated {:?} symbol", slave_symbol);
                }
            }
        }

        for (master_symbol, slave_symbols) in &self.mapping {
            let (real_master_symbol, real_master) = match reverse.get(master_symbol) {
                Some(superior_master_symbol) => (
                    superior_master_symbol.clone(),
                    mapping.get_mut(superior_master_symbol).unwrap(),
                ),
                None => (
                    master_symbol.clone(),
                    mapping.entry(master_symbol.clone()).or_default(),
                ),
            };

            for slave_symbol in slave_symbols {
                if real_master.insert(slave_symbol.clone()) {
                    if reverse.insert(slave_symbol.clone(), real_master_symbol.clone()).is_some() {
                        return Err!("Duplicated {:?} symbol", slave_symbol);
                    }
                }
            }
        }

        *self = PerformanceMergingConfig {mapping, reverse};
        Ok(())
    }
}

impl<'de> Deserialize<'de> for PerformanceMergingConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let mut config = PerformanceMergingConfig::default();

        let mapping = Deserialize::deserialize(deserializer)?;
        config.add_mapping(mapping).map_err(|e| D::Error::custom(format!(
            "Invalid performance merging configuration: {}", e)))?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;
    use rstest::rstest;
    use super::*;

    #[rstest(iterations => [1, 2])]
    fn config_merging(iterations: usize) {
        let mut config = PerformanceMergingConfig::default();

        for _ in 0..iterations {
            config.add_mapping(hashmap!{
                s!("ABC") => hashset!{s!("A"), s!("B")},
                s!("DEF") => hashset!{s!("D"), s!("E")},
                s!("IJ") => hashset!{s!("I"), s!("J")},
            }).unwrap();

            assert_matches!(config.add_mapping(hashmap!{
                s!("LM") => hashset!{s!("L"), s!("M")},
                s!("LN") => hashset!{s!("L"), s!("N")},
            }), Err(e) if e.to_string() == r#"Duplicated "L" symbol"#);

            assert_matches!(config.add_mapping(hashmap!{
                s!("LM") => hashset!{s!("L"), s!("M")},
                s!("LMN") => hashset!{s!("LM"), s!("N")},
            }), Err(e) if e.to_string() == r#"Cycle mapping on "LM" symbol"#);
        }

        config.add_mapping(hashmap!{
            s!("ABC") => hashset!{s!("B"), s!("C")},
            s!("DEF") => hashset!{s!("F")},
            s!("IJK") => hashset!{s!("I"), s!("IJ"), s!("K")},
        }).unwrap();

        assert_eq!(config.mapping, hashmap!{
            s!("ABC") => hashset!{s!("A"), s!("B"), s!("C")},
            s!("DEF") => hashset!{s!("D"), s!("E"), s!("F")},
            s!("IJK") => hashset!{s!("I"), s!("J"), s!("K"), s!("IJ")},
        });

        assert_eq!(config.reverse, hashmap!{
            s!("A") => s!("ABC"),
            s!("B") => s!("ABC"),
            s!("C") => s!("ABC"),

            s!("D") => s!("DEF"),
            s!("E") => s!("DEF"),
            s!("F") => s!("DEF"),

            s!("I") => s!("IJK"),
            s!("J") => s!("IJK"),
            s!("K") => s!("IJK"),
            s!("IJ") => s!("IJK"),
        });
    }
}