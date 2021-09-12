use std::collections::HashMap;
use std::default::Default;

use serde::Deserialize;
use serde::de::Deserializer;

use crate::core::GenericResult;

pub struct InstrumentInternalIds(HashMap<String, String>);

impl Default for InstrumentInternalIds {
    fn default() -> InstrumentInternalIds {
        InstrumentInternalIds(HashMap::new())
    }
}

impl InstrumentInternalIds {
    pub fn get_symbol(&self, id: &str) -> GenericResult<&str> {
        Ok(self.0.get(id).ok_or_else(|| format!(concat!(
            "Unable to determine stock symbol by its internal ID ({}). ",
            "Please specify the mapping via `instrument_internal_ids` configuration option"
        ), id))?)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<InstrumentInternalIds, D::Error>
        where D: Deserializer<'de>
    {
        Ok(InstrumentInternalIds(Deserialize::deserialize(deserializer)?))
    }
}