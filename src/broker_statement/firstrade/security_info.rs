use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityInfoSection {
    #[serde(rename = "SECLIST")]
    security_list: SecurityList,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityList {
    #[serde(rename = "STOCKINFO")]
    stock_info: Vec<StockInfo>,
    #[serde(rename = "OTHERINFO")]
    other_info: Vec<OtherInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StockInfo {
    #[serde(rename = "SECINFO")]
    security_info: SecurityInfo,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OtherInfo {
    #[serde(rename = "SECINFO")]
    security_info: SecurityInfo,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityInfo {
    #[serde(rename = "SECID")]
    id: SecurityId,
    #[serde(rename = "SECNAME")]
    name: String,
    #[serde(rename = "TICKER")]
    symbol: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityId {
    #[serde(rename = "UNIQUEID")]
    id: String,
    #[serde(rename = "UNIQUEIDTYPE")]
    _type: String,
}