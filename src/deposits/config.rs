use serde::Deserialize;

use crate::cash_flow::config::deserialize_cash_flows;
use crate::core::EmptyResult;
use crate::formatting;
use crate::time::deserialize_date;
use crate::types::{Date, Decimal};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepositConfig {
    pub name: String,

    #[serde(deserialize_with = "deserialize_date")]
    pub open_date: Date,
    #[serde(deserialize_with = "deserialize_date")]
    pub close_date: Date,

    #[serde(default)]
    pub currency: Option<String>,
    pub amount: Decimal,
    pub interest: Decimal,
    #[serde(default)]
    pub capitalization: bool,
    #[serde(default, deserialize_with = "deserialize_cash_flows")]
    pub contributions: Vec<(Date, Decimal)>,
}

impl DepositConfig {
    pub fn validate(&self) -> EmptyResult {
        if self.open_date > self.close_date {
            return Err!(
                "Invalid {:?} deposit dates: {} -> {}",
                self.name, formatting::format_date(self.open_date),
                formatting::format_date(self.close_date));
        }

        for &(date, _amount) in &self.contributions {
            if date < self.open_date || date > self.close_date {
                return Err!(
                    "Invalid {:?} deposit contribution date: {}",
                    self.name, formatting::format_date(date));
            }
        }

        Ok(())
    }
}