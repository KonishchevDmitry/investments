use std::iter::Iterator;

use chrono::Duration;

use broker_statement::ib::IbStatementParser;
use broker_statement::ib::common::{Record, RecordParser, parse_date, format_record};
use core::{EmptyResult, GenericResult};
use currency::{Cash, CashAssets};
use types::Date;
use util;

pub struct StatementInfoParser {}

impl RecordParser for StatementInfoParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        if record.get_value("Field Name")? == "Period" {
            let period = record.get_value("Field Value")?;
            let period = parse_period(period)?;
            parser.statement.set_period(period)?;
        }

        Ok(())
    }
}

pub struct NetAssetValueParser {}

impl RecordParser for NetAssetValueParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let asset_class = match record.get_value("Asset Class") {
            // FIXME: We should be able to handle data with different headers somehow
            Err(_) => return Ok(()),
            Ok(asset_class) => asset_class,
        };

        if asset_class == "Cash" || asset_class == "Stock" {
            let currency = "USD"; // FIXME: Get from statement
            let amount = Cash::new_from_string(currency, record.get_value("Current Total")?)?;

            // FIXME: Accumulate in parser?
            // FIXME: Eliminate hacks with Cash type
            parser.statement.total_value = Some(Cash::new(currency, match parser.statement.total_value {
                Some(total_amount) => total_amount.amount + amount.amount,
                None => amount.amount,
            }));
        }

        Ok(())
    }
}

pub struct DepositsParser {}

impl RecordParser for DepositsParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency.starts_with("Total") {
            return Ok(());
        }

        // FIXME: Distinguish withdrawals from deposits
        let date = parse_date(record.get_value("Settle Date")?)?;
        let amount = Cash::new_from_string_positive(currency, record.get_value("Amount")?)?;

        parser.statement.deposits.push(CashAssets::new_from_cash(date, amount));

        Ok(())
    }
}

pub struct FinancialInstrumentInformationParser {
}

impl RecordParser for FinancialInstrumentInformationParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        parser.tickers.insert(
            record.get_value("Symbol")?.to_owned(),
            record.get_value("Description")?.to_owned(),
        );
        Ok(())
    }
}

pub struct UnknownRecordParser {}

impl RecordParser for UnknownRecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn parse(&self, _parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        if false {
            trace!("Data: {}.", format_record(record.values.iter().skip(1)));
        }
        Ok(())
    }
}

fn parse_period(period: &str) -> GenericResult<(Date, Date)> {
    let dates = period.split(" - ").collect::<Vec<_>>();

    return Ok(match dates.len() {
        1 => {
            let date = parse_period_date(dates[0])?;
            (date, date + Duration::days(1))
        },
        2 => {
            let start = parse_period_date(dates[0])?;
            let end = parse_period_date(dates[1])?;

            if start > end {
                return Err!("Invalid period: {} - {}",
                    util::format_date(start), util::format_date(end));
            }

            (start, end + Duration::days(1))
        },
        _ => return Err!("Invalid date: {:?}", period),
    });
}

fn parse_period_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%B %d, %Y")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_parsing() {
        assert_eq!(parse_period("October 1, 2018").unwrap(),
                   (date!(1, 10, 2018), date!(2, 10, 2018)));

        assert_eq!(parse_period("September 30, 2018").unwrap(),
                   (date!(30, 9, 2018), date!(1, 10, 2018)));

        assert_eq!(parse_period("May 21, 2018 - September 28, 2018").unwrap(),
                   (date!(21, 5, 2018), date!(29, 9, 2018)));
    }
}