use std::borrow::Cow;
use std::fs::File;
use std::io::{Read, BufReader};
use std::ops::Deref;
#[cfg(test)] use std::path::Path;

use encoding_rs;
use regex::Regex;

use core::GenericResult;

use self::record::{Record, UnknownRecord};
use self::encoding::{TaxStatementType, Integer};
use self::foreign_income::ForeignIncome;

#[macro_use] mod record;
mod encoding;
mod foreign_income;

#[derive(Debug)]
pub struct TaxStatement {
    pub year: i32,
    records: Vec<Box<Record>>,
}

pub struct TaxStatementParser {
    file: BufReader<File>,
    buffer: Vec<u8>,
}

impl TaxStatementParser {
    pub fn parse(path: &str) -> GenericResult<TaxStatement> {
        lazy_static! {
            static ref extension_regex: Regex = Regex::new(r"\.dc(\d)$").unwrap();
        }

        let year = extension_regex.captures(path)
            .and_then(|captures| captures.get(1).unwrap().as_str().parse::<u8>().ok())
            .ok_or_else(||"Invalid tax statement file extension: *.dcX is expected")?;
        let year = 2010 + (year as i32);

        Ok(TaxStatementParser::parse_impl(year, path).map_err(|e| format!(
            "Error while reading {:?}: {}", path, e))?)
    }

    // FIXME: HERE
    fn parse_impl(year: i32, path: &str) -> GenericResult<TaxStatement> {
        let mut parser = TaxStatementParser {
            file: BufReader::new(File::open(path)?),
            buffer: Vec::new(),
        };

        let header = get_header(year);
        if parser.read(header.len())? != header {
            return Err!("The file has an unexpected header");
        }

        let mut records = Vec::new();
        let mut record_name = parser.read_data()?.deref().to_owned();

        loop {
            let (record, next_record_name) = match record_name.as_str() {
                ForeignIncome::RECORD_NAME => ForeignIncome::parse(&mut parser)?,
                _ => UnknownRecord::parse(&mut parser, record_name)?,
            };

            records.push(record);
            record_name = match next_record_name {
                Some(record_name) => record_name,
                None => parser.read_data()?.deref().to_owned(),
            };

            if record_name == "@Nalog" {
                let mut buffer = [0; 3];

                if parser.read_value::<Integer>()? != 0 ||
                    parser.file.read(&mut buffer[..])? != 2 ||
                    buffer[0..2] != [0, 0] {
                    return Err!("The file has an unexpected footer");
                }

                break;
            }
        }

        let statement = TaxStatement {
            year: year,
            records: records,
        };
        debug!("{:#?}", statement);

        Ok(statement)
    }

    fn read_value<T>(&mut self) -> GenericResult<T> where T: TaxStatementType {
        let data = self.read_data()?;
        let value = TaxStatementType::decode(data.deref())?;
        Ok(value)
    }

    fn read_data(&mut self) -> GenericResult<Cow<str>> {
        let size = self.read_data_size()?;
        let data = self.read(size)?;
        Ok(data)
    }

    fn read_data_size(&mut self) -> GenericResult<usize> {
        let data = self.read(4)?;
        let size = data.parse::<usize>().map_err(|_| format!(
            "Got an invalid record data size: {:?}", data))?;
        Ok(size)
    }

    fn read(&mut self, size: usize) -> GenericResult<Cow<str>> {
        let capacity = self.buffer.capacity();
        if capacity < size {
            self.buffer.reserve(size - capacity);
        }

        unsafe {
            self.buffer.set_len(size);
        }

        self.file.read_exact(&mut self.buffer)?;

        let (data, _, errors) = encoding_rs::WINDOWS_1251.decode(
            self.buffer.as_slice());

        if errors {
            return Err!("Got an invalid Windows-1251 encoded data");
        }

        Ok(data)
    }
}

fn get_header(year: i32) -> String {
    format!(r"DLSG            Decl{}0102FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", year)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let path = Path::new(file!()).parent().unwrap().join("testdata/empty.dc7");
        test_parsing(path.to_str().unwrap());
    }

    #[test]
    fn parse_filled() {
        let path = Path::new(file!()).parent().unwrap().join("testdata/filled.dc7");
        test_parsing(path.to_str().unwrap());
        // FIXME: Check filled data
    }

    #[test]
    fn parse_real() {
        test_parsing("testdata/statement.dc7");
    }

    // FIXME: Test read + write
    fn test_parsing(path: &str) -> TaxStatement {
        let statement = TaxStatementParser::parse(path).unwrap();
        assert_eq!(statement.year, 2017);
        statement
    }
}