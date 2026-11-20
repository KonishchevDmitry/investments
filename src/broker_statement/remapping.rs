use std::collections::HashMap;

use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::exchanges::Exchange;
use crate::time::DateOptTime;

#[derive(Default)]
pub struct SymbolRemappingRules(Vec<SymbolRemappingRule>);

impl<'de> Deserialize<'de> for SymbolRemappingRules {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Rules {
            Map(HashMap<String, String>),
            List(Vec<SymbolRemappingRule>),
        }

        let mapping = match Rules::deserialize(deserializer)? {
            Rules::Map(mapping) => mapping,
            Rules::List(rules) => return Ok(SymbolRemappingRules(rules)),
        };

        let mut rules = Vec::new();

        for (old, new) in &mapping {
            if mapping.contains_key(new) {
                return Err(D::Error::custom(format!(
                    "Invalid symbol remapping configuration: Recursive {old} symbol")));
            }

            rules.push(SymbolRemappingRule {
                from: old.clone(),
                to: new.clone(),
                when: SymbolRemappingTime::Pre,
                exchange: None,
            });
        }

        Ok(SymbolRemappingRules(rules))
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
    pub from: String,
    pub to: String,
    pub when: SymbolRemappingTime,
    pub exchange: Option<Exchange>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all="kebab-case")]
pub enum SymbolRemappingTime {
    Pre,
    Post,
}

#[derive(Clone, Copy)]
pub enum SymbolRenameType {
    CorporateAction{
        time: DateOptTime,
    },
    Remapping {
        exchange: Option<Exchange>,
        check_existence: bool,
    },
}