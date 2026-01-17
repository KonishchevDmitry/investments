use std::collections::{HashMap, hash_map::Entry};
#[cfg(test)] use std::fs;
use std::path::Path;

use crate::core::{GenericResult, EmptyResult};
use crate::formatting::format_date;
use crate::time::{Date, DateTime};
#[cfg(test)] use crate::util;

use super::common::{Record, RecordSpec, format_error_record, is_header_field};

pub type TradeExecutionInfo = HashMap<OrderId, OrderInfo>;

#[derive(PartialEq, Eq, Hash)]
pub struct OrderId {
    pub time: DateTime,
    pub symbol: String,
}

pub struct OrderInfo {
    pub execution_date: Date,
    non_trade: bool,
}

pub fn try_parse(path: &Path, execution_info: &mut TradeExecutionInfo) -> GenericResult<bool> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)?;
    let mut records = reader.records();

    let headers = match records.next() {
        Some(record) => record?,
        None => return Ok(false),
    };
    let headers = headers.iter().collect::<Vec<&str>>();

    // Assume that it's trade confirmation report if it doesn't look like activity report, because
    // activity report has more strict format unlike trade confirmation report which is effectively
    // flex query and may include an arbitrary combination of enabled columns.
    if headers.len() >= 2 && is_header_field(headers[1]) {
        return Ok(false);
    }

    let record_spec = RecordSpec::new("Trade confirmation", headers, 0);

    for record in records {
        let record = record?;
        parse_record(&Record::new(&record_spec, &record), execution_info).map_err(|e| format!(
            "Failed to parse {} record: {}", format_error_record(&record), e
        ))?;
    }

    Ok(true)
}

fn parse_record(record: &Record, execution_dates: &mut TradeExecutionInfo) -> EmptyResult {
    if record.get_value("AssetClass")? != "STK" || record.get_value("LevelOfDetail")? != "EXECUTION" {
        return Ok(());
    }

    let symbol = record.parse_symbol("Symbol")?;
    let conclusion_time = record.parse_date_time("Date/Time")?;
    let order_id = OrderId {
        time: conclusion_time,
        symbol: symbol.clone(),
    };

    // Corporate actions may lead to receiving of fractional shares which are immediately sold out by technical sell
    // operations which may have no settle date (https://github.com/KonishchevDmitry/investments/issues/80).
    let non_trade = record.get_value("SettleDate")?.is_empty() && record.get_value("TradeID")?.is_empty();
    let execution_date = record.parse_date(if non_trade {
        "TradeDate"
    } else {
        "SettleDate"
    })?;

    match execution_dates.entry(order_id) {
        Entry::Occupied(mut entry) => {
            let entry = entry.get_mut();

            if non_trade {
                if entry.non_trade && entry.execution_date < execution_date {
                    entry.execution_date = execution_date;
                }
            } else if entry.non_trade {
                *entry = OrderInfo {execution_date, non_trade}
            } else if entry.execution_date != execution_date {
                return Err!(
                    "Got several execution dates for {} trade on {}: {} and {}",
                    symbol, format_date(conclusion_time), format_date(entry.execution_date), format_date(execution_date));
            }
        },
        Entry::Vacant(entry) => {
            entry.insert(OrderInfo {execution_date, non_trade});
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let mut info = TradeExecutionInfo::new();
        let path = Path::new(file!()).parent().unwrap().join("testdata/empty-trade-confirmation.csv");
        assert!(try_parse(&path, &mut info).unwrap());
        assert!(info.is_empty());
    }

    #[test]
    fn parse_real() {
        let mut count = 0;
        let mut info = TradeExecutionInfo::new();

        for entry in fs::read_dir("testdata/interactive-brokers/my").unwrap() {
            let path = entry.unwrap().path();
            if !util::has_extension(&path, "csv") {
                continue
            }

            if try_parse(&path, &mut info).unwrap() {
                count += 1;
            }
        }

        assert_ne!(count, 0);
        assert!(!info.is_empty());
    }
}