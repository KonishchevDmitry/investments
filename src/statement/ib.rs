use std::io;
use std::iter::Iterator;

use csv::{self, StringRecord, StringRecordIter};

use core::{EmptyResult, GenericResult};

pub struct InteractiveBrokersStatementParser {
    state: State,
}

impl InteractiveBrokersStatementParser {
    pub fn new() -> InteractiveBrokersStatementParser {
        InteractiveBrokersStatementParser {
            state: State::Header,
        }
    }

    pub fn parse(&mut self) -> EmptyResult {
        let mut reader = csv::ReaderBuilder::new().flexible(true).has_headers(false)
            .from_reader(io::stdin());

        for result in reader.records() {
            let record = result?;

            if let State::Data(_) = self.state {
                if is_header(&record) {
                    self.state = State::Header;
                }
            }

            match self.state {
                State::Header => {
                    self.state = State::Data(parse_header(&record)?);
                },
                State::Data(_) => {
                    parse_data(&record)?;
                },
            };
        }

        Ok(())
    }
}

struct DataSpec {
    name: String,
    columns: Vec<String>,
}

enum State {
    Header,
    Data(DataSpec),
}

fn is_header(record: &StringRecord) -> bool {
    record.len() >= 2 && record.get(1).unwrap() == "Header"
}

fn parse_header(record: &StringRecord) -> GenericResult<DataSpec> {
    // HACK
    if !is_header(record) {
        return Err!("Got an invalid header record: {}.", record.iter()
                .map(|value| format!("{:?}", value)).collect::<Vec<_>>().join(","))
    }

    let data_spec = DataSpec {
        name: record.get(0).unwrap().to_owned(),
        columns: record.iter().skip(2).map(|value| value.to_owned()).collect(),
    };

    trace!("Header: {}: {}.", data_spec.name, data_spec.columns.join(", "));

    Ok(data_spec)
}

fn parse_data(record: &StringRecord) -> EmptyResult {
    // HACK
    if record.len() < 2 || !match record.get(1).unwrap() {
        "Data" | "Total" | "SubTotal" => true,
        _ => false,
    } {
        return Err!("Got an invalid data record: {}.", record.iter()
                .map(|value| format!("{:?}", value)).collect::<Vec<_>>().join(","))
    }

//    let data_spec = DataSpec {
//        name: record.get(0).unwrap().to_owned(),
//        values: record.iter().skip(2).map(|value| value.to_owned()).collect(),
//    };

    trace!("Data: {}.", format_record(record.iter().skip(2)));

    Ok(())
}

fn format_record<'a, I>(iter: I) -> String where I: IntoIterator<Item = &'a str> {
    iter.into_iter().map(|value| format!("{:?}", value)).collect::<Vec<_>>().join(", ")
}