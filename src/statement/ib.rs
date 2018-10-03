use std::io;
use std::iter::Iterator;

use chrono::Duration;

use csv::{self, StringRecord};

use core::{EmptyResult, GenericResult};
use currency::Cash;
use statement::{Statement, StatementBuilder, Transaction};
use types::Date;

pub struct IbStatementParser {
    statement: StatementBuilder,
}

impl IbStatementParser {
    pub fn new() -> IbStatementParser {
        IbStatementParser {
            statement: StatementBuilder::new(),
        }
    }

    pub fn parse(mut self) -> GenericResult<Statement> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(io::stdin());

        let mut records = reader.records();
        let mut next_header_record = {
            if let Some(result) = records.next() {
                let record = result?;

                if !is_header(&record) {
                    return Err!("Invalid header record: {}", format_record(&record));
                }

                Some(record)
            } else {
                return Err!("The statement file is empty");
            }
        };

        'header: loop {
            let header_record = next_header_record.take().unwrap();
            let (name, fields) = parse_header(&header_record)?;

            let parser: Box<RecordParser> = match name {
                "Statement" => Box::new(StatementInfoParser {}),
                "Deposits & Withdrawals" => Box::new(DepositsInfoParser {}),
                _ => Box::new(UnknownRecordParser {}),
            };

            let data_types = parser.data_types();

            while let Some(result) = records.next() {
                let record = result?;

                if is_header(&record) {
                    next_header_record = Some(record);
                    continue 'header;
                }

                if record.len() < 2 || record.get(0).unwrap() != name {
                    return Err!("Invalid data record where {:?} data record is expected: {}",
                                name, format_record(&record));
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

        Ok(self.statement.get().map_err(|e| format!("Invalid statement: {}", e))?)
    }
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

fn is_header(record: &StringRecord) -> bool {
    record.len() >= 2 && record.get(1).unwrap() == "Header"
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

struct DepositsInfoParser {}

impl RecordParser for DepositsInfoParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency.starts_with("Total") {
            return Ok(());
        }

        // FIXME: Distinguish withdrawals from deposits
        let date = parse_transaction_date(record.get_value("Settle Date")?)?;
        let amount = Cash::new_from_string(currency, record.get_value("Amount")?)?;

        parser.statement.deposits.push(Transaction::new(date, amount));

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
    parse_date(date, "%B %d, %Y")
}

fn parse_transaction_date(date: &str) -> GenericResult<Date> {
    parse_date(date, "%Y-%m-%d")
}

fn parse_date(date: &str, format: &str) -> GenericResult<Date> {
    Ok(Date::parse_from_str(date, format).map_err(|_| format!(
        "Invalid date: {:?}", date))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_parsing() {
        assert_eq!(parse_period("October 1, 2018").unwrap(),
                   (Date::from_ymd(2018, 10, 1), Date::from_ymd(2018, 10, 2)));

        assert_eq!(parse_period("September 30, 2018").unwrap(),
                   (Date::from_ymd(2018, 9, 30), Date::from_ymd(2018, 10, 1)));

        assert_eq!(parse_period("May 21, 2018 - September 28, 2018").unwrap(),
                   (Date::from_ymd(2018, 5, 21), Date::from_ymd(2018, 9, 29)));
    }

    #[test]
    fn transaction_date_parsing() {
        assert_eq!(parse_transaction_date("2018-06-22").unwrap(), Date::from_ymd(2018, 6, 22));
    }
}