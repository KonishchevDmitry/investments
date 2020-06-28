use serde::Deserialize;

use super::common::Ignore;
use super::security_info::SecurityInfoSection;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OFX {
    #[serde(rename = "SIGNONMSGSRSV1")]
    signon: Ignore,

    #[serde(rename = "INVSTMTMSGSRSV1")]
    statement: StatementSection,

    #[serde(rename = "SECLISTMSGSRSV1")]
    security_info: SecurityInfoSection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatementSection {
    #[serde(rename = "INVSTMTTRNRS")]
    response: StatementResponse,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatementResponse {
    #[serde(rename = "TRNUID")]
    id: Ignore,
    #[serde(rename = "STATUS")]
    status: Ignore,
    #[serde(rename = "INVSTMTRS")]
    statement: Statement,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Statement {
    // FIXME(konishchev): Support
    #[serde(rename = "DTASOF")]
    date: String,
    #[serde(rename = "CURDEF")]
    currency: String,
    #[serde(rename = "INVACCTFROM")]
    account: Ignore,
    #[serde(rename = "INVTRANLIST")]
    transactions: Transactions,
    #[serde(rename = "INVBAL")]
    balance: Ignore,
    #[serde(rename = "INVPOSLIST")]
    open_positions: Ignore,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transactions {
    #[serde(rename = "DTSTART")]
    start_date: String,
    #[serde(rename = "DTEND")]
    end_date: String,

    // FIXME(konishchev): Support
    #[serde(rename = "INVBANKTRAN")]
    deposits: Vec<Ignore>,
    #[serde(rename = "BUYSTOCK")]
    stock_buys: Vec<Ignore>,
    #[serde(rename = "SELLSTOCK")]
    stock_sells: Vec<Ignore>,
    #[serde(rename = "INCOME")]
    income: Vec<Ignore>,
}