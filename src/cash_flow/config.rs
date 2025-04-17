use std::collections::HashMap;

use serde::Deserialize;
use serde::de::{Deserializer, Error};

use crate::time;
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

pub fn deserialize_cash_flows<'de, D>(deserializer: D) -> Result<Vec<(Date, Decimal)>, D::Error>
    where D: Deserializer<'de>
{
    let deserialized: HashMap<String, Decimal> = Deserialize::deserialize(deserializer)?;
    let mut cash_flows = Vec::new();

    for (date, amount) in deserialized {
        let date = time::parse_user_date(&date).map_err(D::Error::custom)?;
        let amount = util::validate_decimal(amount, DecimalRestrictions::StrictlyPositive).map_err(|_|
            D::Error::custom(format!("Invalid amount: {:?}", amount)))?;

        cash_flows.push((date, amount));
    }

    cash_flows.sort_by_key(|cash_flow| cash_flow.0);

    Ok(cash_flows)
}