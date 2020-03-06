use crate::broker_statement::trades::{StockBuy, StockSell};
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date_time};

pub struct OpenPositionsParser {}

impl RecordParser for OpenPositionsParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["Total", "Notes"])
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

        Ok(())
    }
}

pub struct TradesParser {}

impl RecordParser for TradesParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["SubTotal", "Total"])
    }

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        record.check_value("DataDiscriminator", "Order")?;

        if record.get_value("Asset Category")? == "Forex" {
            return Ok(());
        }

        record.check_value("Asset Category", "Stocks")?;

        let currency = record.get_value("Currency")?;
        let symbol = record.get_value("Symbol")?;
        let quantity: i32 = record.parse_value("Quantity")?;

        let conclusion_date = parse_date_time(record.get_value("Date/Time")?)?.date();
        let execution_date = parser.get_execution_date(symbol, conclusion_date);

        let price = Cash::new(
            currency, record.parse_cash("T. Price", DecimalRestrictions::StrictlyPositive)?);
        let commission = -record.parse_cash("Comm/Fee", DecimalRestrictions::NegativeOrZero)?;

        // Commission may be 1.06 in *.pdf but 1.0619736 in *.csv, so round it to cents
        let commission = Cash::new(currency, commission).round();

        if quantity > 0 {
            parser.statement.stock_buys.push(StockBuy::new(
                symbol, quantity as u32, price, commission, conclusion_date, execution_date));
        } else if quantity < 0 {
            parser.statement.stock_sells.push(StockSell::new(
                symbol, -quantity as u32, price, commission, conclusion_date, execution_date, false));
        } else {
            return Err!("Invalid quantity: {}", quantity)
        }

        Ok(())
    }
}
