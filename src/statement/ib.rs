use std::io;
use std::iter::Iterator;

use csv::{self, StringRecord};

use core::{EmptyResult, GenericResult};

pub struct InteractiveBrokersStatementParser {}

impl InteractiveBrokersStatementParser {
    pub fn new() -> InteractiveBrokersStatementParser {
        InteractiveBrokersStatementParser {}
    }

    pub fn parse(&mut self) -> EmptyResult {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(io::stdin());

        let mut records = reader.records();

        let mut record = {
            if let Some(result) = records.next() {
                result?
            } else {
                return Err!("The statement file is empty");
            }
        };

        'header: loop {
            let parser = {
                let (name, fields) = parse_header(&record)?;

                let parser: Box<RecordParser> = match name {
                    "Statement" => Box::new(StatementInfoParser {}),
                    _ => Box::new(UnknownRecordParser {}),
                };

                if let Some(expected_fields) = parser.fields() {
                    if fields != expected_fields {
                        return Err!("{:?} header has an unexpected fields: {}",
                                name, format_record(fields.iter().map(|field: &&str| *field)));
                    }
                }

                parser
            };

            while let Some(result) = records.next() {
                record = result?;

                if is_header(&record) {
                    continue 'header;
                }

                parser.parse(&record)?;
            }

            break;
        }

        Ok(())
    }
}

trait RecordParser {
    fn fields(&self) -> Option<Vec<&str>>;
    fn parse(&self, record: &StringRecord) -> EmptyResult;
}

fn is_header(record: &StringRecord) -> bool {
    record.len() >= 2 && record.get(1).unwrap() == "Header"
}

fn parse_header<'a>(record: &'a StringRecord) -> GenericResult<(&'a str, Vec<&'a str>)> {
    if !is_header(record) {
        return Err!("Invalid header record: {}", format_record(record.iter()));
    }

    let name = record.get(0).unwrap();
    let fields = record.iter().skip(2).collect::<Vec<_>>();
    trace!("Header: {}: {}.", name, format_record(fields.iter().map(|field: &&str| *field)));

    Ok((name, fields))
}

// HACK: HERE
struct StatementInfoParser {}

impl RecordParser for StatementInfoParser {
    fn fields(&self) -> Option<Vec<&str>> {
        Some(vec!["Field Name", "Field Value"])
    }

    fn parse(&self, record: &StringRecord) -> EmptyResult {
        #[derive(Deserialize)]
        struct Info {
            name: String,
            value: String,
        }

        let info: Info = record.deserialize(None)?;
        error!("{}: {}", info.name, info.value);
        Ok(())
    }
}

fn parse_data(record: &StringRecord) -> EmptyResult {
    // HACK
    if record.len() < 2 || !match record.get(1).unwrap() {
        "Data" | "Total" | "SubTotal" => true,
        _ => false,
    } {
        return Err!("Invalid data record: {}", format_record(record.iter()));
    }

    //    let data_spec = DataSpec {
    //        name: record.get(0).unwrap().to_owned(),
    //        values: record.iter().skip(2).map(|value| value.to_owned()).collect(),
    //    };

    trace!("Data: {}.", format_record(record.iter().skip(2)));

    Ok(())
}

fn format_record<'a, I>(iter: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    iter.into_iter()
        .map(|value| format!("{:?}", value))
        .collect::<Vec<_>>()
        .join(", ")
}

struct UnknownRecordParser {}

impl RecordParser for UnknownRecordParser {
    fn fields(&self) -> Option<Vec<&str>> {
        None
    }

    fn parse(&self, record: &StringRecord) -> EmptyResult {
        Ok(())
    }
}
