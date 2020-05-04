use std::ops::Deref;

use crate::broker_statement::trades::{ForexTrade, StockBuy, StockSell};
use crate::core::EmptyResult;
use crate::currency;
use crate::types::Date;
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

        let asset_category = record.get_value("Asset Category")?;
        let symbol = record.get_value("Symbol")?;
        let conclusion_date = parse_date_time(record.get_value("Date/Time")?)?.date();

        match asset_category {
            "Forex" => parse_forex_record(parser, record, symbol, conclusion_date),
            "Stocks" => parse_stock_record(parser, record, symbol, conclusion_date),
            _ => return Err!("Unsupported asset category: {}", asset_category)
        }
    }
}

fn parse_forex_record(
    parser: &mut StatementParser, record: &Record, symbol: &str, conclusion_date: Date
) -> EmptyResult {
    let pair: Vec<&str> = symbol.split('.').collect();
    if pair.len() != 2 {
        return Err!("Invalid forex pair: {}", symbol)
    }

    let base = pair.first().unwrap().deref();
    let quote = pair.last().unwrap().deref();

    // Please note: The value is actually may be rounded which leads to inaccuracy in cash flow
    // report calculation.
    let quantity = record.parse_cash("Quantity", base, DecimalRestrictions::NonZero)?;

    let volume = record.parse_cash("Proceeds", quote, DecimalRestrictions::NonZero)?;
    let commission = -record.parse_cash("Comm in USD", "USD", DecimalRestrictions::NegativeOrZero)?;

    let (from, to) = if quantity.is_positive() {
        (-volume, quantity)
    } else {
        (-quantity, volume)
    };
    if from.is_negative() || to.is_negative() {
        return Err!("Unexpected Forex quantity/volume values: {}/{}", quantity, volume);
    }

    parser.statement.forex_trades.push(ForexTrade{from, to, commission, conclusion_date});

    Ok(())
}

fn parse_stock_record(
    parser: &mut StatementParser, record: &Record, symbol: &str, conclusion_date: Date,
) -> EmptyResult {
    let currency = record.get_value("Currency")?;
    let quantity: i32 = record.parse_value("Quantity")?;
    let price = record.parse_cash("T. Price", currency, DecimalRestrictions::StrictlyPositive)?;
    let commission = -record.parse_cash("Comm/Fee", currency, DecimalRestrictions::NegativeOrZero)?;
    let execution_date = parser.get_execution_date(symbol, conclusion_date);

    let volume = record.parse_cash("Proceeds", currency, if quantity < 0 {
        DecimalRestrictions::StrictlyPositive
    } else {
        DecimalRestrictions::StrictlyNegative
    })?;
    debug_assert_eq!(volume.amount, currency::round_to((price * -quantity).amount, 4));

    if quantity > 0 {
        parser.statement.stock_buys.push(StockBuy::new(
            symbol, quantity as u32, price, -volume, commission, conclusion_date, execution_date));
    } else if quantity < 0 {
        parser.statement.stock_sells.push(StockSell::new(
            symbol, -quantity as u32, price, volume, commission, conclusion_date, execution_date, false));
    } else {
        return Err!("Invalid quantity: {}", quantity)
    }

    Ok(())
}