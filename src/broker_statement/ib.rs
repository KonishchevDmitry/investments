use std::iter::Iterator;

use chrono::Duration;

use csv::{self, StringRecord};

use core::{EmptyResult, GenericResult};
use currency::{Cash, CashAssets};
use broker_statement::{BrokerStatement, BrokerStatementBuilder};
use types::Date;
use util;

pub struct IbStatementParser {
    statement: BrokerStatementBuilder,
}

impl IbStatementParser {
    pub fn new() -> IbStatementParser {
        IbStatementParser {
            statement: BrokerStatementBuilder::new(),
        }
    }

    pub fn parse(mut self, path: &str) -> GenericResult<BrokerStatement> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(path)?;

        let mut records = reader.records();
        let mut state = Some(State::None);

        'state: loop {
            match state.take().unwrap() {
                State::None => {
                    match records.next() {
                        Some(result) => state = Some(State::Record(result?)),
                        None => break,
                    };
                }
                State::Record(record) => {
                    if record.len() < 2 {
                        return Err!("Invalid record: {}", format_record(&record));
                    }

                    if record.get(1).unwrap() == "Header" {
                        state = Some(State::Header(record));
                    } else if record.get(1).unwrap() == "" {
                        trace!("Headerless record: {}.", format_record(&record));
                        state = Some(State::None);
                    } else {
                        return Err!("Invalid record: {}", format_record(&record));
                    }
                },
                State::Header(record) => {
                    let (name, fields) = parse_header(&record)?;

                    let parser: Box<RecordParser> = match name {
                        "Statement" => Box::new(StatementInfoParser {}),
                        "Net Asset Value" => Box::new(NetAssetValueParser {}),
                        "Deposits & Withdrawals" => Box::new(DepositsParser {}),
                        _ => Box::new(UnknownRecordParser {}),
                    };

                    let data_types = parser.data_types();

                    while let Some(result) = records.next() {
                        let record = result?;

                        if record.len() < 2 {
                            return Err!("Invalid record: {}", format_record(&record));
                        }

                        if record.get(0).unwrap() != name {
                            state = Some(State::Record(record));
                            continue 'state;
                        } else if record.get(1).unwrap() == "Header" {
                            state = Some(State::Header(record));
                            continue 'state;
                        }

                        if let Some(data_types) = data_types {
                            if !data_types.contains(&record.get(1).unwrap()) {
                                return Err!("Invalid data record type: {}", format_record(&record));
                            }
                        }

                        parser.parse(&mut self, &Record {
                            name: name,
                            fields: &fields,
                            values: &record,
                        }).map_err(|e| format!(
                            "Failed to parse ({}) record: {}", format_record(&record), e
                        ))?;
                    }

                    break;
                }
            }
        }

        Ok(self.statement.get().map_err(|e| format!("Invalid statement: {}", e))?)
    }
}

enum State {
    None,
    Record(StringRecord),
    Header(StringRecord),
}

struct Record<'a> {
    name: &'a str,
    fields: &'a Vec<&'a str>,
    values: &'a StringRecord,
}

impl<'a> Record<'a> {
    fn get_value(&self, field: &str) -> GenericResult<&str> {
        if let Some(index) = self.fields.iter().position(|other: &&str| *other == field) {
            if let Some(value) = self.values.get(index + 2) {
                return Ok(value);
            }
        }

        Err!("{:?} record doesn't have {:?} field", self.name, field)
    }
}

fn parse_header(record: &StringRecord) -> GenericResult<(&str, Vec<&str>)> {
    let name = record.get(0).unwrap();
    let fields = record.iter().skip(2).collect::<Vec<_>>();
    trace!("Header: {}: {}.", name, format_record(fields.iter().map(|field: &&str| *field)));
    Ok((name, fields))
}

trait RecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> { Some(&["Data"]) }
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult;
}

struct StatementInfoParser {}

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

struct NetAssetValueParser {}

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

struct DepositsParser {}

impl RecordParser for DepositsParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency.starts_with("Total") {
            return Ok(());
        }

        // FIXME: Distinguish withdrawals from deposits
        let date = parse_transaction_date(record.get_value("Settle Date")?)?;
        let amount = Cash::new_from_string(currency, record.get_value("Amount")?)?;

        parser.statement.deposits.push(CashAssets::new_from_cash(date, amount));

        Ok(())
    }
}

struct UnknownRecordParser {}

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

fn format_record<'a, I>(iter: I) -> String
    where I: IntoIterator<Item = &'a str> {

    iter.into_iter()
        .map(|value| format!("{:?}", value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_period(period: &str) -> GenericResult<(Date, Date)> {
    let dates = period.split(" - ").collect::<Vec<_>>();

    return Ok(match dates.len() {
        1 => {
            let date = parse_period_date(dates[0])?;
            (date, date + Duration::days(1))
        },
        2 => (parse_period_date(dates[0])?, parse_period_date(dates[1])? + Duration::days(1)),
        _ => return Err!("Invalid date: {:?}", period),
    });
}

fn parse_period_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%B %d, %Y")
}

fn parse_transaction_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%Y-%m-%d")
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

    #[test]
    fn transaction_date_parsing() {
        assert_eq!(parse_transaction_date("2018-06-22").unwrap(), date!(22, 6, 2018));
    }
}