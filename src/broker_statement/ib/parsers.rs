use std::iter::Iterator;

use chrono::Duration;
use log::trace;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::formatting;
use crate::types::Date;
use crate::util::{self, DecimalRestrictions};

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date, format_record};

pub struct StatementInfoParser {}

impl RecordParser for StatementInfoParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        if record.get_value("Field Name")? == "Period" {
            let period = record.get_value("Field Value")?;
            let period = parse_period(period)?;
            parser.statement.set_period(period)?;
        }

        Ok(())
    }
}

pub struct AccountInformationParser {}

impl RecordParser for AccountInformationParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let name = record.get_value("Field Name")?;
        let value = record.get_value("Field Value")?;

        if name == "Account Capabilities" {
            if value != "Cash" {
                return Err!("Unsupported account type: {}", value);
            }
        } else if name == "Base Currency" {
            parser.base_currency.replace(value.to_owned());
        }

        Ok(())
    }
}

pub struct ChangeInNavParser {}

impl RecordParser for ChangeInNavParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        if record.get_value("Field Name")? == "Starting Value" {
            let currency = parser.base_currency.as_ref().ok_or_else(||
                "Unable to determine account base currency")?;

            let amount = Cash::new_from_string(&currency, record.get_value("Field Value")?)?;
            parser.statement.set_starting_assets(!amount.is_zero())?;
        }

        Ok(())
    }
}

pub struct CashReportParser {}

impl RecordParser for CashReportParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        if record.get_value("Currency Summary")? != "Ending Cash" {
            return Ok(());
        }

        let currency = record.get_value("Currency")?;
        let amount = record.parse_cash("Total", DecimalRestrictions::PositiveOrZero)?;

        record.check_value("Futures", "0")?;
        record.check_value("Total", record.get_value("Securities")?)?;

        if currency == "Base Currency Summary" {
            let currency = parser.base_currency.as_ref().ok_or_else(||
                "Unable to determine account base currency")?;

            if parser.base_currency_summary.replace(Cash::new(&currency, amount)).is_some() {
                return Err!("Got duplicated base currency summary");
            }
        } else {
            if parser.statement.cash_assets.has_assets(currency) {
                return Err!("Got duplicated {} assets", currency);
            }

            parser.statement.cash_assets.deposit(Cash::new(currency, amount));
        }

        Ok(())
    }
}

pub struct DepositsAndWithdrawalsParser {}

impl RecordParser for DepositsAndWithdrawalsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        let date = parse_date(record.get_value("Settle Date")?)?;
        let amount = record.parse_cash("Amount", DecimalRestrictions::NonZero)?;

        parser.statement.cash_flows.push(
            CashAssets::new_from_cash(date, Cash::new(currency, amount)));

        Ok(())
    }
}

pub struct FinancialInstrumentInformationParser {
}

impl RecordParser for FinancialInstrumentInformationParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let symbol = record.get_value("Symbol")?;

        if parser.statement.instrument_names.insert(
            symbol.to_owned(), record.get_value("Description")?.to_owned()).is_some() {
            return Err!("Duplicated symbol: {}", symbol);
        }

        Ok(())
    }
}

pub struct UnknownRecordParser {}

impl RecordParser for UnknownRecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn parse(&self, _parser: &mut StatementParser, record: &Record) -> EmptyResult {
        if false {
            trace!("Data: {}.", format_record(record.values.iter().skip(1)));
        }
        Ok(())
    }
}

fn parse_period(period: &str) -> GenericResult<(Date, Date)> {
    let dates = period.split(" - ").collect::<Vec<_>>();

    Ok(match dates.len() {
        1 => {
            let date = parse_period_date(dates[0])?;
            (date, date + Duration::days(1))
        },
        2 => {
            let start = parse_period_date(dates[0])?;

            let mut end = parse_period_date(dates[1])?;
            end += Duration::days(1);

            if start >= end {
                return Err!("Invalid period: {}", formatting::format_period(start, end));
            }

            (start, end)
        },
        _ => return Err!("Invalid date: {:?}", period),
    })
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