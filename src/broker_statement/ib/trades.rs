use crate::broker_statement::StockBuy;
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date_time};

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

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        record.check_value("DataDiscriminator", "Order")?;

        // TODO: Here we should calculate taxes from currency selling
        if record.get_value("Asset Category")? == "Forex" {
            return Ok(());
        }

        record.check_value("Asset Category", "Stocks")?;

        let currency = record.get_value("Currency")?;
        let symbol = record.get_value("Symbol")?;
        let date = parse_date_time(record.get_value("Date/Time")?)?.date();

        let quantity: i32 = record.parse_value("Quantity")?;
        if quantity == 0 {
            return Err!("Invalid quantity: {}", quantity)
        } else if quantity < 0 {
            // TODO: Support position closing
            return Err!("Position closing is not supported yet");
        }

        let price = Cash::new(
            currency, record.parse_cash("T. Price", DecimalRestrictions::StrictlyPositive)?);
        let commission = Cash::new(
            currency, record.parse_cash("Comm/Fee", DecimalRestrictions::NegativeOrZero)?);

        parser.statement.stock_buys.push(StockBuy {
            date: date,
            symbol: symbol.to_owned(),
            quantity: quantity as u32,
            price: price,
            commission: commission,
            sold: 0,
        });

        return Ok(());
    }
}
