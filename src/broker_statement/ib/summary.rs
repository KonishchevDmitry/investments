use std::iter::Iterator;

use log::warn;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::time;
use crate::types::Date;

use super::StatementParser;
use super::common::{Record, RecordParser};

pub struct StatementInfoParser {}

impl RecordParser for StatementInfoParser {
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
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
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let name = record.get_value("Field Name")?;
        let value = record.get_value("Field Value")?;

        if name == "Account Capabilities" {
            match value {
                "Cash" => {},
                "Margin" => {
                    if *parser.warn_on_margin_account {
                        warn!(concat!(
                            "The program is not expected to work properly with margin accounts ",
                            "(see https://github.com/KonishchevDmitry/investments/issues/8), ",
                            "so be critical to its calculation results."
                        ));
                        *parser.warn_on_margin_account = false;
                    }
                },
                _ => return Err!("Unsupported account type: {}", value),
            }
        } else if name == "Base Currency" {
            parser.base_currency.replace(value.to_owned());
        }

        Ok(())
    }
}

pub struct ChangeInNavParser {}

impl RecordParser for ChangeInNavParser {
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        if record.get_value("Field Name")? == "Starting Value" {
            let currency = parser.base_currency()?;
            let amount = Cash::new_from_string(currency, record.get_value("Field Value")?)?;
            parser.statement.set_starting_assets(!amount.is_zero())?;
        }

        Ok(())
    }
}

fn parse_period(period: &str) -> GenericResult<(Date, Date)> {
    let dates = period.split(" - ").collect::<Vec<_>>();

    Ok(match dates.len() {
        1 => {
            let date = parse_period_date(dates[0])?;
            (date, date.succ())
        },
        2 => {
            let start = parse_period_date(dates[0])?;
            let end = parse_period_date(dates[1])?;
            time::parse_period(start, end)?
        },
        _ => return Err!("Invalid date: {:?}", period),
    })
}

fn parse_period_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%B %d, %Y")
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