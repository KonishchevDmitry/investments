use std::collections::HashMap;

use serde::Deserialize;

use crate::core::{EmptyResult, GenericResult};
use crate::formatting::format_date;
use crate::time::deserialize_date;
use crate::types::Date;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaxRemappingConfig {
    #[serde(deserialize_with = "deserialize_date")]
    pub date: Date,
    pub description: String,
    #[serde(deserialize_with = "deserialize_date")]
    pub to_date: Date,
}

pub struct TaxRemapping {
    remapping: HashMap<(Date, String), (Date, bool)>
}

impl TaxRemapping {
    pub fn new() -> TaxRemapping {
        TaxRemapping {
            remapping: HashMap::new(),
        }
    }

    pub fn from_config(configs: &[TaxRemappingConfig]) -> GenericResult<TaxRemapping> {
        let mut remapping = TaxRemapping::new();

        for config in configs {
            remapping.add(config.date, &config.description, config.to_date)?;
        }

        Ok(remapping)
    }

    pub fn add(&mut self, date: Date, description: &str, to_date: Date) -> EmptyResult {
        if self.remapping.insert((date, description.to_owned()), (to_date, false)).is_some() {
            return Err!(
                "Invalid tax remapping configuration: Duplicated match: {} - {:?}",
                format_date(date), description);
        }
        Ok(())
    }

    pub fn map(&mut self, date: Date, description: &str) -> Date {
        if let Some((to_date, mapped)) = self.remapping.get_mut(&(date, description.to_owned())) {
            *mapped = true;
            *to_date
        } else {
            date
        }
    }

    pub fn ensure_all_mapped(&self) -> EmptyResult {
        for ((date, description), (_, mapped)) in self.remapping.iter() {
            if !mapped {
                return Err!(
                    "The following tax remapping rule hasn't been mapped to any tax: {} - {:?}",
                    format_date(*date), description)
            }
        }

        Ok(())
    }
}