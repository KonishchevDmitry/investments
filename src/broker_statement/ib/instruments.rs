use chrono::Datelike;

use crate::core::EmptyResult;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{self, Record, RecordParser, SecurityID, parse_symbol};

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
        // If symbol renames save its ISIN the column contains both symbols
        // (see https://github.com/KonishchevDmitry/investments/issues/29)

        let symbols = record.get_value("Symbol")?.split(',').map(str::trim)
            .skip_while(|symbol| symbol.ends_with(common::OLD_SYMBOL_SUFFIX));

        for symbol in symbols {
            let symbol = parse_symbol(symbol)?;

            // It may be duplicated when the security changes its ID due to corporate action (stock
            // split for example).
            let instrument = parser.statement.instrument_info.get_or_add(&symbol);
            instrument.set_name(record.get_value("Description")?);

            let security_id = record.get_value("Security ID")?;
            if security_id.is_empty() {
                if parser.statement.get_period()?.first_date().year() < 2020 {
                    // Old broker statements provide only conid (in a separate field)
                } else {
                    return Err!("Security ID is missing for {}", symbol);
                }
            } else {
                match security_id.parse::<SecurityID>()? {
                    SecurityID::Isin(isin) => instrument.add_isin(isin),
                    SecurityID::Cusip(cusip) => instrument.add_cusip(cusip),
                    _ => {
                        return Err!(
                            "Got an unsupported security ID for {}: {:?}",
                            symbol, security_id);
                    }
                }
            }
        }

        Ok(())
    }
}