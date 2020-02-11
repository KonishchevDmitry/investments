use std::collections::HashMap;
#[cfg(test)] use std::fs;
#[cfg(test)] use std::path::Path;

use csv;

use crate::core::{GenericResult};
use crate::formatting::format_date;
use crate::types::Date;

use super::common::{Record, RecordSpec, parse_date};

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
        let record = Record::new(&record_spec, &record);

        if record.get_value("AssetClass")? != "STK" {
            continue;
        }

        if record.get_value("LevelOfDetail")? != "EXECUTION" {
            continue;
        }

        let symbol = record.get_value("Symbol")?;
        let conclusion_date = parse_date(record.get_value("TradeDate")?)?;
        let execution_date = parse_date(record.get_value("SettleDate")?)?;

        match execution_dates.insert(OrderId {
            symbol: symbol.to_owned(),
            date: conclusion_date,
        }, execution_date) {
            Some(other_date) if other_date != execution_date => {
                return Err!("Got several execution dates for {} trade on {}: {} and {}",
                    symbol, format_date(conclusion_date), format_date(execution_date),
                    format_date(other_date));
            },
            _ => {},
        }
    }

    Ok(true)
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

        // FIXME: testdata/interactive-brokers/current
        for entry in fs::read_dir(".").unwrap() {
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