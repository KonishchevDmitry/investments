use serde::Deserialize;

use crate::core::GenericResult;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::types::Date;
use crate::util;

use super::balance::Balance;
use super::common::{Ignore, deserialize_date};
use super::security_info::SecurityInfoSection;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OFX {
    #[serde(rename = "SIGNONMSGSRSV1")]
    signon: Ignore,

    #[serde(rename = "INVSTMTMSGSRSV1")]
    statement: StatementSection,

    // FIXME(konishchev): Support all below
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
    // FIXME(konishchev): Support all below
    #[serde(rename = "DTASOF", deserialize_with = "deserialize_date")]
    date: Date,
    #[serde(rename = "CURDEF")]
    currency: String,
    #[serde(rename = "INVACCTFROM")]
    account: Ignore,
    #[serde(rename = "INVTRANLIST")]
    transactions: Transactions,
    #[serde(rename = "INVBAL")]
    balance: Balance,
    #[serde(rename = "INVPOSLIST")]
    open_positions: Ignore,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transactions {
    #[serde(rename = "DTSTART", deserialize_with = "deserialize_date")]
    start_date: Date,
    #[serde(rename = "DTEND", deserialize_with = "deserialize_date")]
    end_date: Date,

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

impl OFX {
    // FIXME(konishchev): Implement
    pub fn parse(self) -> GenericResult<PartialBrokerStatement> {
        println!("{:#?}", self); // FIXME(konishchev): Remove

        let report = self.statement.response.statement;
        let currency = report.currency;
        let transactions = report.transactions;

        let (start_date, end_date) = util::parse_period(
            transactions.start_date, transactions.end_date)?;

        if report.date < start_date || end_date <= report.date {
            // The report contains transactions for the specified period, but balance and open
            // positions info - for the statement generation date.
            return Err!(concat!(
                "Firstrade reports always must be generated for the period starting from account ",
                "opening date until the date when the report is generated"))
        }

        // FIXME(konishchev): Implement
        let mut statement = PartialBrokerStatement::new();
        statement.set_period((start_date, end_date))?;
        statement.set_starting_assets(false)?;
        report.balance.parse(&mut statement, &currency)?;

        statement.validate()
    }
}