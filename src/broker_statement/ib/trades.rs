use crate::broker_statement::trades::{ForexTrade, StockBuy, StockSell};
use crate::core::EmptyResult;
use crate::time::DateTime;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, check_volume, parse_symbol};

pub struct TradesParser {}

impl RecordParser for TradesParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["SubTotal", "Total"])
    }

    fn allow_multiple(&self) -> bool {
        true
    }

    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let data_type_field = "DataDiscriminator";
        match record.get_value(data_type_field)? {
            // Default Activity Statement contains only this type
            "Order" => {},

            // Custom Activity Statement types:
            // * Trade - order execution details (one order may be executed via several trades)
            // * ClosedLot - closed positions calculation
            "Trade" | "ClosedLot" => return Ok(()),

            value => return Err!("Got an unexpected {:?} field value: {:?}", data_type_field, value),
        };

        let asset_category = record.get_value("Asset Category")?;
        let symbol = record.get_value("Symbol")?;
        let conclusion_time = record.parse_date_time("Date/Time")?;

        match asset_category {
            "Forex" => parse_forex_record(parser, record, symbol, conclusion_time),
            "Stocks" => parse_stock_record(parser, record, symbol, conclusion_time),
            _ => Err!("Unsupported asset category: {}", asset_category),
        }
    }
}

fn parse_forex_record(
    parser: &mut StatementParser, record: &Record, symbol: &str, conclusion_date: DateTime
) -> EmptyResult {
    let pair: Vec<&str> = symbol.split('.').collect();
    if pair.len() != 2 {
        return Err!("Invalid forex pair: {}", symbol)
    }

    let base = *pair.first().unwrap();
    let quote = *pair.last().unwrap();
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
        &format!("Comm in {commission_currency}"),
        commission_currency, DecimalRestrictions::NegativeOrZero)?;

    parser.statement.forex_trades.push(ForexTrade::new(
        conclusion_date.into(), from, to, commission));

    Ok(())
}

fn parse_stock_record(
    parser: &mut StatementParser, record: &Record, symbol: &str, conclusion_time: DateTime,
) -> EmptyResult {
    let symbol = parse_symbol(symbol)?;
    let currency = record.get_value("Currency")?;
    let price = record.parse_cash("T. Price", currency, DecimalRestrictions::StrictlyPositive)?;
    let commission = -record.parse_cash("Comm/Fee", currency, DecimalRestrictions::NegativeOrZero)?;
    let execution_date = parser.get_execution_date(&symbol, conclusion_time);
    let quantity = record.parse_quantity("Quantity", DecimalRestrictions::NonZero)?;

    let volume = record.parse_cash("Proceeds", currency, if quantity.is_sign_positive() {
        DecimalRestrictions::StrictlyNegative
    } else {
        DecimalRestrictions::StrictlyPositive
    })?;
    check_volume(-quantity, price, volume)?;

    if quantity.is_sign_positive() {
        parser.statement.stock_buys.push(StockBuy::new_trade(
            &symbol, quantity, price, -volume, commission,
            conclusion_time.into(), execution_date));
    } else {
        parser.statement.stock_sells.push(StockSell::new_trade(
            &symbol, -quantity, price, volume, commission,
            conclusion_time.into(), execution_date, false));
    }

    Ok(())
}