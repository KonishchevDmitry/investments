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
        let mut next_header_record = {
            if let Some(result) = records.next() {
                let record = result?;

                if !is_header(&record) {
                    return Err!("Invalid header record: {}", format_record_iter(record.iter()));
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
                _ => Box::new(UnknownRecordParser {}),
            };

            let data_types = parser.data_types();

            // HACK
//            if let Some(expected_fields) = parser.fields() {
//                if fields != expected_fields {
//                    return Err!("{:?} header has an unexpected fields: {}",
//                            name, format_record(fields.iter().map(|field: &&str| *field)));
//                }
//            }

            while let Some(result) = records.next() {
                let record = result?;

                if is_header(&record) {
                    next_header_record = Some(record);
                    continue 'header;
                }

                if record.len() < 1 || record.get(0).unwrap() != name {
                    return Err!("Invalid data record where {:?} data record is expected: {}",
                                name, format_record(&record));
                }

                if record.len() != fields.len() + 2 {
                    return Err!(concat!(
                        "Invalid data record: ({}). The number of values doesn't match the number ",
                        "of fields in the header."
                    ), format_record(&record))
                }

                if let Some(data_types) = data_types {
                    if !data_types.contains(&record.get(1).unwrap()) {
                        return Err!("Invalid data record type: {}", format_record(&record));
                    }
                }

                parser.parse(&Record {
                    name: name,
                    fields: &fields,
                    values: record,
                })?;
            }

            break;
        }

        Ok(())
    }
}

struct Record<'a> {
    name: &'a str,
    fields: &'a Vec<&'a str>,
    values: StringRecord,
}

impl<'a> Record<'a> {
    fn get_value(&self, field: &str) -> GenericResult<&str> {
        if let Some(index) = self.fields.iter().position(|other: &&str| *other == field) {
            return Ok(self.values.get(2 + index).unwrap());
        } else {
            return Err!("{:?} record doesn't have {:?} field", self.name, field)
        }
    }
}

fn is_header(record: &StringRecord) -> bool {
    record.len() >= 2 && record.get(1).unwrap() == "Header"
}

fn parse_header<'a>(record: &'a StringRecord) -> GenericResult<(&'a str, Vec<&'a str>)> {
    let name = record.get(0).unwrap();
    let fields = record.iter().skip(2).collect::<Vec<_>>();
    trace!("Header: {}: {}.", name, format_record_iter(fields.iter().map(|field: &&str| *field)));
    Ok((name, fields))
}

trait RecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> { Some(&["Data"]) }
    fn parse(&self, record: &Record) -> EmptyResult;
}

// HACK: HERE
struct StatementInfoParser {}

impl RecordParser for StatementInfoParser {
    fn parse(&self, record: &Record) -> EmptyResult {
        if record.get_value("Field Name")? != "Period" {
            return Ok(());
        }

        let period = record.get_value("Field Value")?;
        error!(">>> {}", period);
//        let name = record.get();
//        #[derive(Deserialize)]
//        struct Info {
//            name: String,
//            value: String,
//        }
//
//        let info: Info = record.deserialize(None)?;
//        error!("{}: {}", info.name, info.value);
        Ok(())
    }
}

fn parse_data(record: &StringRecord) -> EmptyResult {
    // HACK
    if record.len() < 2 || !match record.get(1).unwrap() {
        "Data" | "Total" | "SubTotal" => true,
        _ => false,
    } {
        return Err!("Invalid data record: {}", format_record(&record));
    }

    //    let data_spec = DataSpec {
    //        name: record.get(0).unwrap().to_owned(),
    //        values: record.iter().skip(2).map(|value| value.to_owned()).collect(),
    //    };

    trace!("Data: {}.", format_record_iter(record.iter().skip(2)));

    Ok(())
}

fn format_record(record: &StringRecord) -> String {
    format_record_iter(record.iter())
}

fn format_record_iter<'a, I>(iter: I) -> String
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
//    fn fields(&self) -> Option<&'static [&'static str]> {
//        None
//    }

    fn data_types(&self) -> Option<&'static [&'static str]> { None }
    fn parse(&self, record: &Record) -> EmptyResult {
        Ok(())
    }
}
