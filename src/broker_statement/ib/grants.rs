use crate::broker_statement::grants::StockGrant;
use crate::core::EmptyResult;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser};

pub struct GrantsParser {}

impl RecordParser for GrantsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let symbol = record.parse_symbol("Symbol")?;
        let date = record.parse_date("Report Date")?;
        let quantity = record.parse_quantity("Quantity", DecimalRestrictions::StrictlyPositive)?;
        parser.statement.stock_grants.push(StockGrant::new(date, &symbol, quantity));
        Ok(())
    }
}