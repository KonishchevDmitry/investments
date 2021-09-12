use crate::core::EmptyResult;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_symbol};

pub struct OpenPositionsParser {}

impl RecordParser for OpenPositionsParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["Total"])
    }

    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let data_type_field = "DataDiscriminator";
        match record.get_value(data_type_field)? {
            // Default Activity Statement contains only this type
            "Summary" => record.check_values(&[
                ("Asset Category", "Stocks"),
                ("Mult", "1"),
            ])?,

            // Custom Activity Statement types:
            // * Lot - open position calculation
            "Lot" => return Ok(()),

            value => return Err!("Got an unexpected {:?} field value: {:?}", data_type_field, value),
        };

        let symbol = record.parse_symbol("Symbol")?;
        let quantity = record.parse_quantity("Quantity", DecimalRestrictions::StrictlyPositive)?;
        parser.statement.add_open_position(&symbol, quantity)
    }
}

pub struct FinancialInstrumentInformationParser {
}

impl RecordParser for FinancialInstrumentInformationParser {
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        // If symbol renames saving its ISIN the column contains both symbols
        // (see https://github.com/KonishchevDmitry/investments/issues/29)

        for symbol in record.get_value("Symbol")?.split(',').map(str::trim) {
            if symbol.ends_with(".OLD") {
                continue;
            }

            let symbol = parse_symbol(symbol)?;
            let name = record.get_value("Description")?;

            // It may be duplicated when the security changes its ID due to corporate action (stock
            // split for example).
            parser.statement.instrument_info.get_or_add(&symbol).set_name(name);
        }

        Ok(())
    }
}