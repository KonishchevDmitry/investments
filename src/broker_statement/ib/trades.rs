use std::str::FromStr;

use core::EmptyResult;
use currency::Cash;
use types::Decimal;

use super::IbStatementParser;
use super::common::{Record, RecordParser, CashType, parse_time};

pub struct OpenPositionsParser {}

impl RecordParser for OpenPositionsParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["Total"])
    }

    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        record.check_values(&[
            ("DataDiscriminator", "Summary"),
            ("Asset Category", "Stocks"),
            ("Mult", "1"),
        ])?;

        let symbol = record.get_value("Symbol")?;
        let quantity = record.parse_value("Quantity")?;

        if parser.statement.open_positions.insert(symbol.to_owned(), quantity).is_some() {
            return Err!("Got a duplicated {:?} symbol", symbol);
        }

        return Ok(());
    }
}

pub struct TradesParser {}

impl RecordParser for TradesParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["SubTotal", "Total"])
    }

    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        record.check_value("DataDiscriminator", "Order")?;

        // TODO: Taxes from selling?
        if record.get_value("Asset Category")? == "Forex" {
            return Ok(());
        }

        let quantity = record.get_value("Quantity")?.replace(',', "");
        let quantity = Decimal::from_str(&quantity).map_err(|_| "Invalid quantity")?;
        if quantity.is_sign_negative() {
            // TODO: Support selling
            return Err!("Position closing is not supported yet");
        }

        record.check_value("Asset Category", "Stocks")?;

        let currency = record.get_value("Currency")?;
        let ticker = record.get_value("Symbol")?;
        let date = parse_time(record.get_value("Date/Time")?)?.date();
        let quantity: u32 = record.parse_value("Quantity")?;
        let price = Cash::new_from_string_positive(currency, record.get_value("T. Price")?)?;

//        let commission = record.parse_value("Comm/Fee")?;
//        if commission.is_sign_positive() {
//            return Err!("Invalid commission: {:?}", commission);
//        }
//        let commission = Cash::new(currency, commission);

        return Ok(());
    }
}
