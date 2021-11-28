use serde::Deserialize;

use crate::core::{GenericResult, EmptyResult};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::exchanges::Exchange;
use crate::time::{Date, Period};

use super::StatementReader;
use super::balance::Balance;
use super::common::{Ignore, deserialize_date};
use super::open_positions::OpenPositions;
use super::security_info::SecurityInfoSection;
use super::transactions::Transactions;

pub struct StatementParser<'a> {
    pub reader: &'a mut StatementReader,
    pub statement: PartialBrokerStatement,
    is_last: bool,
}

impl<'a> StatementParser<'a> {
    pub fn parse(reader: &mut StatementReader, statement: Ofx, is_last: bool) -> GenericResult<PartialBrokerStatement> {
        let mut parser = StatementParser {
            reader,
            statement: PartialBrokerStatement::new(&[Exchange::Us], false),
            is_last,
        };
        statement.parse(&mut parser)?;
        parser.statement.validate()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ofx {
    #[serde(rename = "SIGNONMSGSRSV1")]
    _signon: Ignore,

    #[serde(rename = "INVSTMTMSGSRSV1")]
    statement: StatementSection,

    #[serde(rename = "SECLISTMSGSRSV1")]
    security_info: SecurityInfoSection,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatementSection {
    #[serde(rename = "INVSTMTTRNRS")]
    response: StatementResponse,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatementResponse {
    #[serde(rename = "TRNUID")]
    _id: Ignore,
    #[serde(rename = "STATUS")]
    _status: Ignore,
    #[serde(rename = "INVSTMTRS")]
    report: Report,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    #[serde(rename = "DTASOF", deserialize_with = "deserialize_date")]
    date: Date,
    #[serde(rename = "CURDEF")]
    currency: String,
    #[serde(rename = "INVACCTFROM")]
    _account: Ignore,
    #[serde(rename = "INVTRANLIST")]
    transactions: Transactions,
    #[serde(rename = "INVPOSLIST")]
    open_positions: OpenPositions,
    #[serde(rename = "INVBAL")]
    balance: Balance,
}

impl Ofx {
    pub fn parse(self, parser: &mut StatementParser) -> EmptyResult {
        // Attention:
        //
        // Firstrade reports contain transactions for the specified period, but balance and open
        // positions are always calculated for the statement generation date.

        let report = self.statement.response.report;
        let currency = report.currency;
        let transactions = report.transactions;

        let period = Period::new(transactions.start_date, transactions.end_date)?;
        if parser.is_last && period.last_date() < report.date {
            return Err!("Last Firstrade report must include the day when the report was generated")
        }
        parser.statement.set_period(period)?;

        parser.statement.set_has_starting_assets(false)?;
        if parser.is_last {
            report.balance.parse(parser, &currency)?;
        }

        let securities = self.security_info.parse()?;
        transactions.parse(parser, &currency, &securities)?;
        if parser.is_last {
            report.open_positions.parse(parser, &securities)?;
        }

        Ok(())
    }
}