use std::collections::HashMap;
#[cfg(test)] use std::fs;
#[cfg(test)] use std::path::Path;

use crate::core::{GenericResult, EmptyResult};
use crate::formatting::format_date;
use crate::types::Date;

use super::common::{Record, RecordSpec, format_error_record};

pub type TradeExecutionDates = HashMap<OrderId, Date>;

#[derive(PartialEq, Eq, Hash)]
pub struct OrderId {
    pub symbol: String,
    pub date: Date,
}

pub fn try_parse(path: &str, execution_dates: &mut TradeExecutionDates) -> GenericResult<bool> {
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
    if headers.len() >= 2 && headers[1] == "Header" {
        return Ok(false);
    }

    let record_spec = RecordSpec::new("Trade confirmation", headers, 0);

    for record in records {
        let record = record?;
        parse_record(&Record::new(&record_spec, &record), execution_dates).map_err(|e| format!(
            "Failed to parse {} record: {}", format_error_record(&record), e
        ))?;
    }

    Ok(true)
}

fn parse_record(record: &Record, execution_dates: &mut TradeExecutionDates) -> EmptyResult {
    if record.get_value("AssetClass")? != "STK" ||
        record.get_value("LevelOfDetail")? != "EXECUTION" {
        return Ok(());
    }

    let symbol = record.parse_symbol("Symbol")?;
    let conclusion_date = record.parse_date("TradeDate")?;
    let execution_date = record.parse_date("SettleDate")?;

    match execution_dates.insert(OrderId {
        symbol: symbol.clone(),
        date: conclusion_date,
    }, execution_date) {
        Some(other_date) if other_date != execution_date => {
            return Err!("Got several execution dates for {} trade on {}: {} and {}",
                symbol, format_date(conclusion_date), format_date(execution_date),
                format_date(other_date));
        },
        _ => {},
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let mut execution_dates = TradeExecutionDates::new();
        let path = Path::new(file!()).parent().unwrap().join(
            "testdata/empty-trade-confirmation.csv");
        assert!(try_parse(path.to_str().unwrap(), &mut execution_dates).unwrap());
        assert!(execution_dates.is_empty());
    }

    #[test]
    fn parse_real() {
        let mut count = 0;
        let mut execution_dates = TradeExecutionDates::new();

        for entry in fs::read_dir("testdata/interactive-brokers/my").unwrap() {
            let path = entry.unwrap().path();
            let path = path.to_str().unwrap();

            if !path.ends_with(".csv") {
                continue
            }

            if try_parse(path, &mut execution_dates).unwrap() {
                count += 1;
            }
        }

        assert_ne!(count, 0);
        assert!(!execution_dates.is_empty());
    }
}