use crate::core::EmptyResult;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser};

pub struct OpenPositionsParser {}

impl RecordParser for OpenPositionsParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["Total"])
    }

    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        record.check_values(&[
            ("DataDiscriminator", "Summary"),
            ("Asset Category", "Stocks"),
            ("Mult", "1"),
        ])?;

        let symbol = record.get_value("Symbol")?;
        let quantity = record.parse_quantity("Quantity", DecimalRestrictions::StrictlyPositive)?;

        parser.statement.add_open_position(symbol, quantity)
    }
}

pub struct FinancialInstrumentInformationParser {
}

impl RecordParser for FinancialInstrumentInformationParser {
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let symbol = record.get_value("Symbol")?;

        // It may be duplicated when the security changes its ID due to corporate action (stock
        // split for example).
        parser.statement.instrument_names.insert(
            symbol.to_owned(), record.get_value("Description")?.to_owned());

        Ok(())
    }
}