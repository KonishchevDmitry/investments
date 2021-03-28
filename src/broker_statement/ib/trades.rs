use std::ops::Deref;

use crate::broker_statement::trades::{ForexTrade, StockBuy, StockSell, StockSellType, StockSource};
use crate::core::EmptyResult;
use crate::types::Date;
use crate::util::{self, DecimalRestrictions};

use super::StatementParser;
use super::common::{Record, RecordParser};

pub struct TradesParser {}

impl RecordParser for TradesParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["SubTotal", "Total"])
    }

    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        record.check_value("DataDiscriminator", "Order")?;

        let asset_category = record.get_value("Asset Category")?;
        let symbol = record.get_value("Symbol")?;
        let conclusion_date = record.parse_date_time("Date/Time")?.date();

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
    let volume = record.parse_cash("Proceeds", quote, DecimalRestrictions::NonZero)?;

    // Please note: The value is actually may be rounded which leads to inaccuracy in cash flow
    // report calculation.
    let quantity = record.parse_cash("Quantity", base, DecimalRestrictions::NonZero)?;

    let (from, to) = if quantity.is_positive() {
        (-volume, quantity)
    } else {
        (-quantity, volume)
    };
    if from.is_negative() || to.is_negative() {
        return Err!("Unexpected Forex quantity/volume values: {}/{}", quantity, volume);
    }

    let commission_currency = parser.base_currency()?;
    let commission = -record.parse_cash(
        &format!("Comm in {}", commission_currency),
        commission_currency, DecimalRestrictions::NegativeOrZero)?;

    parser.statement.forex_trades.push(ForexTrade{from, to, commission, conclusion_date});

    Ok(())
}

fn parse_stock_record(
    parser: &mut StatementParser, record: &Record, symbol: &str, conclusion_date: Date,
) -> EmptyResult {
    let currency = record.get_value("Currency")?;
    let price = record.parse_cash("T. Price", currency, DecimalRestrictions::StrictlyPositive)?;
    let commission = -record.parse_cash("Comm/Fee", currency, DecimalRestrictions::NegativeOrZero)?;
    let execution_date = parser.get_execution_date(symbol, conclusion_date);

    let quantity = record.get_value("Quantity")?;
    let quantity = util::parse_decimal(quantity, DecimalRestrictions::NonZero).map_err(|_| format!(
        "Got an unexpected {} trade quantity: {}", symbol, quantity))?.normalize();

    let volume = record.parse_cash("Proceeds", currency, if quantity.is_sign_positive() {
        DecimalRestrictions::StrictlyNegative
    } else {
        DecimalRestrictions::StrictlyPositive
    })?;
    if cfg!(debug_assertions) {
        let mut ok = false;
        let expected_volume = price * -quantity;

        for precision in 4..=8 {
            if expected_volume.round_to(precision) == volume {
                ok = true;
                break;
            }
        }

        debug_assert!(ok, "Got an unexpected volume {} vs {}", volume, expected_volume);
    }

    if quantity.is_sign_positive() {
        parser.statement.stock_buys.push(StockBuy::new(
            symbol, quantity, StockSource::Trade, price, -volume, commission,
            conclusion_date, execution_date, false));
    } else {
        parser.statement.stock_sells.push(StockSell::new(
            StockSellType::Trade, symbol, -quantity, price, volume, commission,
            conclusion_date, execution_date, false, false));
    }

    Ok(())
}