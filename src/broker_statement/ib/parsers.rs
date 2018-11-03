use std::iter::Iterator;

use chrono::Duration;

use core::{EmptyResult, GenericResult};
use currency::{Cash, CashAssets};
use types::Date;
use util::{self, DecimalRestrictions};

use super::IbStatementParser;
use super::common::{Record, RecordParser, parse_date, format_record};

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

pub struct ChangeInNavParser {}

impl RecordParser for ChangeInNavParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        if record.get_value("Field Name")? == "Starting Value" {
            let amount = Cash::new_from_string(
                parser.currency, record.get_value("Field Value")?)?;

            parser.statement.set_starting_value(amount)?;
        }

        Ok(())
    }
}

pub struct CashReportParser {}

impl RecordParser for CashReportParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Base Currency Summary" ||
            record.get_value("Currency Summary")? != "Ending Cash" {
            return Ok(());
        }

        if parser.statement.cash_assets.has_assets(currency) {
            return Err!("Got duplicated {} assets", currency);
        }

        record.check_value("Futures", "0")?;
        record.check_value("Total", record.get_value("Securities")?)?;

        let amount = record.parse_cash("Total", DecimalRestrictions::PositiveOrZero)?;
        parser.statement.cash_assets.deposit(Cash::new(currency, amount));

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

        // TODO: Distinguish withdrawals from deposits
        let date = parse_date(record.get_value("Settle Date")?)?;
        let amount = Cash::new(
            currency, record.parse_cash("Amount", DecimalRestrictions::StrictlyPositive)?);

        parser.statement.deposits.push(CashAssets::new_from_cash(date, amount));

        Ok(())
    }
}

pub struct FinancialInstrumentInformationParser {
}

impl RecordParser for FinancialInstrumentInformationParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        parser.statement.instrument_names.insert(
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