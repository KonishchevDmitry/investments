// FIXME: HERE: Mockup rewrite

use std::borrow::Cow;
use std::fs::File;
use std::io::{Read, BufReader};
use std::ops::Deref;
use std::path::Path;

use encoding_rs;
use regex::Regex;

use core::{GenericResult, EmptyResult};
use self::record::{Record, UnknownRecord, is_record_name}; // FIXME

#[macro_use] mod record;
mod foreign_income;
mod encoding;

pub struct TaxStatement {
    year: i32,
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

        let expected_header = format!(
            "DLSG            Decl{}0102FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", year);

        parser.read(expected_header.len())?;

        let mut record_name = parser.read_data()?.deref().to_owned();

        loop {
            let record_parser = match record_name.as_str() {
                "@DeclForeign" => foreign_income::ForeignIncome::parse,
                _ => UnknownRecord::parse,
            };
            let (record, next_record_name) = record_parser(&mut parser, record_name)?;
            debug!("{:?}", record);

            record_name = match next_record_name {
                Some(record_name) => record_name,
                None => parser.read_data()?.deref().to_owned(),
            };

            if record_name == "@Nalog" {
                break;
            }
        }

        Ok(TaxStatement {
            year: year,
        })
    }

    // FIXME: HERE
    fn read_value<T: encoding::TaxStatementType>(&mut self) -> GenericResult<T> {
        let data = self.read_data()?;
        Ok(encoding::TaxStatementType::decode(data.deref())?)
    }

    fn read_data(&mut self) -> GenericResult<Cow<str>> {
        let size = self.read_data_size()?;
        Ok(self.read(size)?)
    }

    fn read_data_size(&mut self) -> GenericResult<usize> {
        let data = self.read(4)?;
        let size = data.parse::<usize>().map_err(|_| format!(
            "Got an invalid record data size: {:?}", data))?;
        Ok(size)
    }

    // FIXME: HERE
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        let path = Path::new(file!()).parent().unwrap().join("testdata/statement.dc7");
        let statement = TaxStatementParser::parse(path.to_str().unwrap()).unwrap();
        assert_eq!(statement.year, 2017);
    }
}