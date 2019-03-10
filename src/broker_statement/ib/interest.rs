use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::broker_statement::interest::IdleCashInterest;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date};

pub struct InterestParser {}

impl RecordParser for InterestParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Total" {
            return Ok(());
        }

        let date = parse_date(record.get_value("Date")?)?;
        let amount = Cash::new(
            currency, record.parse_cash("Amount", DecimalRestrictions::StrictlyPositive)?);

        parser.statement.idle_cash_interest.push(IdleCashInterest {
            date: date,
            amount: amount,
        });

        Ok(())
    }
}