use crate::core::EmptyResult;
use crate::broker_statement::interest::IdleCashInterest;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser};

pub struct InterestParser {}

impl RecordParser for InterestParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        let date = record.parse_date("Date")?;
        let amount = record.parse_cash("Amount", currency, DecimalRestrictions::NonZero)?;
        parser.statement.idle_cash_interest.push(IdleCashInterest::new(date, amount));
        Ok(())
    }
}