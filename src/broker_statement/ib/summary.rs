use std::iter::Iterator;

use log::warn;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::time::{self, Date, Period};
use crate::util::DecimalRestrictions;

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
                        // https://github.com/KonishchevDmitry/investments/issues/8
                        let url = "http://bit.ly/investments-margin-accounts";
                        warn!(concat!(
                            "The program is not expected to work properly with margin accounts ",
                            "(see {}), so be critical to its calculation results."
                        ), url);
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

pub struct NavParser {}

impl RecordParser for NavParser {
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let asset_class = record.get_value("Asset Class")?.trim_end();

        let currency = parser.base_currency()?.to_owned();
        parser.statement.assets.other.get_or_insert_with(|| Cash::zero(&currency));

        match asset_class {
            "Cash" | "Dividend Accruals" | "Interest Accruals"| "Total" => {},
            "Stock" => {
                if !record.parse_amount("Current Short", DecimalRestrictions::No)?.is_zero() {
                    return Err!("Short positions aren't supported")
                }

                let amount = record.parse_amount(
                    "Current Total", DecimalRestrictions::PositiveOrZero)?;

                parser.statement.assets.other.as_mut().unwrap().amount += amount;
            },
            _ => return Err!("Unsupported asset class: {:?}", asset_class),
        }

        Ok(())
    }
}

pub struct ChangeInNavParser {}

impl RecordParser for ChangeInNavParser {
    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        if record.get_value("Field Name")? == "Starting Value" {
            let amount = record.parse_amount("Field Value", DecimalRestrictions::No)?;
            parser.statement.set_has_starting_assets(!amount.is_zero())?;
        }

        Ok(())
    }
}

fn parse_period(period: &str) -> GenericResult<Period> {
    let dates = period.split(" - ").collect::<Vec<_>>();

    Ok(match dates.len() {
        1 => {
            let date = parse_period_date(dates[0])?;
            Period::new(date, date)?
        },
        2 => {
            let start = parse_period_date(dates[0])?;
            let end = parse_period_date(dates[1])?;
            Period::new(start, end)?
        },
        _ => return Err!("Invalid date: {:?}", period),
    })
}

fn parse_period_date(date: &str) -> GenericResult<Date> {
    time::parse_date(date, "%B %d, %Y")
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(input, first, last,
        case("October 1, 2018", date!(2018, 10, 1), date!(2018, 10, 1)),
        case("September 30, 2018", date!(2018, 9, 30), date!(2018, 9, 30)),
        case("May 21, 2018 - September 28, 2018", date!(2018, 5, 21), date!(2018, 9, 28)),
    )]
    fn period_parsing(input: &str, first: Date, last: Date) {
        let period = parse_period(input).unwrap();
        assert_eq!(period.first_date(), first);
        assert_eq!(period.last_date(), last);
    }
}