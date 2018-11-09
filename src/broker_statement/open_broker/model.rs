use types::Date;

use super::parsers::deserialize_date;

#[derive(Deserialize)]
pub struct BrokerReport {
    #[serde(deserialize_with = "deserialize_date")]
    pub date_from: Date,

    #[serde(deserialize_with = "deserialize_date")]
    pub date_to: Date,
}