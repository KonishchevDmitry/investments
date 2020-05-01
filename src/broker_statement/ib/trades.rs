use crate::broker_statement::trades::{ForexTrade, StockBuy, StockSell};
use crate::core::EmptyResult;
use crate::currency::{self, Cash};
use crate::types::{Date, Decimal};
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date_time};
use std::ops::Deref;

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
        let price = record.parse_amount("T. Price", DecimalRestrictions::StrictlyPositive)?;
        let conclusion_date = parse_date_time(record.get_value("Date/Time")?)?.date();

        match asset_category {
            "Forex" => parse_forex_record(parser, record, symbol, price, conclusion_date),
            "Stocks" => parse_stock_record(parser, record, symbol, price, conclusion_date),
            _ => return Err!("Unsupported asset category: {}", asset_category)
        }
    }
}

// FIXME(konishchev): Rewrite
fn parse_forex_record(
    parser: &mut StatementParser, record: &Record,
    symbol: &str, price: Decimal, conclusion_date: Date
) -> EmptyResult {
    let pair: Vec<&str> = symbol.split('.').collect();
    if pair.len() != 2 {
        return Err!("Invalid forex pair: {}", symbol)
    }

    let base = pair.first().unwrap().deref();
    let quote = pair.last().unwrap().deref();

    let quantity = record.parse_amount("Quantity", DecimalRestrictions::NonZero)?;
    let commission = -record.parse_cash("Comm in USD", "USD", DecimalRestrictions::NegativeOrZero)?;

    // FIXME(konishchev): A temporary check during cash flow report developing
    let volume = record.parse_amount("Proceeds", DecimalRestrictions::NonZero)?;
    // debug_assert_eq!(-quantity, currency::round_to(volume / price, 4));

    parser.statement.forex_trades.push(ForexTrade{
        base: base.to_owned(),
        quote: quote.to_owned(),

        quantity: quantity,
        price: price,
        volume: volume,
        commission: commission,

        conclusion_date: conclusion_date,
        execution_date: conclusion_date, // FIXME(konishchev): Not implemented yet
    });

    Ok(())
}

fn parse_stock_record(
    parser: &mut StatementParser, record: &Record,
    symbol: &str, price: Decimal, conclusion_date: Date,
) -> EmptyResult {
    let currency = record.get_value("Currency")?;
    let quantity: i32 = record.parse_value("Quantity")?;
    let execution_date = parser.get_execution_date(symbol, conclusion_date);

    let price = Cash::new(currency, price);
    let mut commission = -record.parse_cash("Comm/Fee", currency, DecimalRestrictions::NegativeOrZero)?;

    // FIXME(konishchev): This may be a problem for cash flow report
    // Commission may be 1.06 in *.pdf but 1.0619736 in *.csv, so round it to cents
    commission = commission.round();

    // FIXME(konishchev): A temporary check during cash flow report developing
    let volume = record.parse_cash("Proceeds", currency, DecimalRestrictions::NonZero)?;
    debug_assert_eq!(volume.amount, -currency::round_to((price * quantity).amount, 4));

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