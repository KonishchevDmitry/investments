use std::collections::HashMap;

use serde::Deserialize;
use serde::de::{Deserializer, Error};
use serde_yaml::Value;

use crate::time::DateOptTime;

#[derive(Default)]
pub struct SymbolRemappingRules(Vec<SymbolRemappingRule>);

// FIXME(konishchev): Document it
impl<'de> Deserialize<'de> for SymbolRemappingRules {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value: Value = Deserialize::deserialize(deserializer)?;

        Ok(SymbolRemappingRules(if value.is_mapping() {
            let mapping: HashMap<String, String> = Deserialize::deserialize(value).map_err(D::Error::custom)?;

            let mut rules = Vec::new();

            for (old, new) in &mapping {
                if mapping.contains_key(new) {
                    return Err(D::Error::custom(format!(
                        "Invalid symbol remapping configuration: Recursive {old} symbol")));
                }

                rules.push(SymbolRemappingRule {
                    old: old.clone(),
                    new: new.clone(),
                });
            }

            rules
        } else {
            Deserialize::deserialize(value).map_err(|e| D::Error::custom(e.to_string()))?
        }))
    }
}

impl<'a> IntoIterator for &'a SymbolRemappingRules {
    type Item = &'a SymbolRemappingRule;
    type IntoIter = std::slice::Iter<'a, SymbolRemappingRule>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Deserialize)]
pub struct SymbolRemappingRule {
    pub old: String,
    pub new: String,
}

#[derive(Clone, Copy)]
pub enum SymbolRenameType {
    CorporateAction{
        time: DateOptTime,
    },
    Remapping {
        check_existence: bool,
        allow_override: bool,
    },
}