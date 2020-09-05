use crate::core::EmptyResult;
use crate::util::{self, DecimalRestrictions};

use super::StatementParser;
use super::common::{Record, RecordParser};

pub struct OpenPositionsParser {}

impl RecordParser for OpenPositionsParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["Total"])
    }

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        record.check_values(&[
            ("DataDiscriminator", "Summary"),
            ("Asset Category", "Stocks"),
            ("Mult", "1"),
        ])?;

        let symbol = record.get_value("Symbol")?;

        let quantity = record.get_value("Quantity")?;
        let quantity = util::parse_decimal(
            quantity, DecimalRestrictions::StrictlyPositive
        ).map_err(|_| format!("Got an unexpected {} quantity: {}", symbol, quantity))?;

        parser.statement.add_open_position(symbol, quantity)
    }
}

pub struct FinancialInstrumentInformationParser {
}

impl RecordParser for FinancialInstrumentInformationParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let symbol = record.get_value("Symbol")?;

        if parser.statement.instrument_names.insert(
            symbol.to_owned(), record.get_value("Description")?.to_owned()).is_some() {
            return Err!("Duplicated symbol: {}", symbol);
        }

        Ok(())
    }
}