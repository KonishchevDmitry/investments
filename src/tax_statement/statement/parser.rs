use std::borrow::Cow;
use std::fs::File;
use std::io::{Read, BufReader, Write, BufWriter};
use std::ops::Deref;
#[cfg(test)] use std::path::Path;
use std::rc::Rc;

use encoding_rs;
use log::debug;
use regex::Regex;
#[cfg(test)] use tempfile::NamedTempFile;

use crate::core::{EmptyResult, GenericResult};

use super::TaxStatement;
use super::record::{Record, UnknownRecord};
use super::encoding::{TaxStatementType, TaxStatementPrimitiveType};
use super::foreign_income::ForeignIncome;
use super::types::Integer;

pub struct TaxStatementReader {
    file: BufReader<File>,
    buffer: Vec<u8>,
}

impl TaxStatementReader {
    pub fn read(path: &str) -> GenericResult<TaxStatement> {
        lazy_static! {
            static ref extension_regex: Regex = Regex::new(r"\.dc(\d)$").unwrap();
        }

        let year = extension_regex.captures(path)
            .and_then(|captures| captures.get(1).unwrap().as_str().parse::<u8>().ok())
            .ok_or_else(||"Invalid tax statement file extension: *.dcX is expected")?;
        let year = 2010 + (year as i32);

        let mut reader = TaxStatementReader {
            file: BufReader::new(File::open(path)?),
            buffer: Vec::new(),
        };

        let header = get_header(year);
        if reader.read_raw(header.len())? != header {
            return Err!("The file has an unexpected header");
        }

        let mut records = Vec::new();
        let mut last_record = false;
        let mut next_record_name = None;

        while !last_record {
            let record_name = match next_record_name.take() {
                Some(record_name) => record_name,
                None => reader.read_data()?.deref().to_owned(),
            };

            let record: Box<Record> = match record_name.as_str() {
                ForeignIncome::RECORD_NAME => Box::new(ForeignIncome::read(&mut reader)?),
                Nalog::RECORD_NAME => {
                    last_record = true;
                    Box::new(Nalog::read(&mut reader)?)
                },
                _ => {
                    let (record, read_next_record_name) = UnknownRecord::read(&mut reader, record_name)?;
                    next_record_name = Some(read_next_record_name);
                    Box::new(record)
                },
            };

            records.push(record);
        }

        let mut footer_buffer = [0; 3];
        if reader.file.read(&mut footer_buffer[..])? != 2 || footer_buffer[0..2] != [0, 0] {
            return Err!("The file has an unexpected footer");
        }

        let statement = TaxStatement {
            path: path.to_owned(),
            year: year,
            records: records,
        };
        debug!("{:#?}", statement);

        Ok(statement)
    }

    pub fn read_value<T>(&mut self) -> GenericResult<T> where T: TaxStatementType {
        TaxStatementType::read(self)
    }

    pub fn read_primitive<T>(&mut self) -> GenericResult<T> where T: TaxStatementPrimitiveType {
        let data = self.read_data()?;
        let value = TaxStatementPrimitiveType::decode(data.deref())?;
        Ok(value)
    }

    pub fn read_data(&mut self) -> GenericResult<Cow<str>> {
        let size = self.read_data_size()?;
        let data = self.read_raw(size)?;
        Ok(data)
    }

    fn read_data_size(&mut self) -> GenericResult<usize> {
        let data = self.read_raw(4)?;
        let size = data.parse::<usize>().map_err(|_| format!(
            "Got an invalid record data size: {:?}", data))?;
        Ok(size)
    }

    fn read_raw(&mut self, size: usize) -> GenericResult<Cow<str>> {
        if self.buffer.len() < size {
            let additional_space = size - self.buffer.len();
            self.buffer.reserve(additional_space);
        }

        unsafe {
            self.buffer.set_len(size);
        }

        self.file.read_exact(&mut self.buffer)?;

        let (data, _, errors) = encoding_rs::WINDOWS_1251.decode(self.buffer.as_slice());
        if errors {
            return Err!("Got an invalid Windows-1251 encoded data");
        }

        Ok(data)
    }
}

pub struct TaxStatementWriter {
    file: BufWriter<File>,
    buffer: Rc<String>,
}

impl TaxStatementWriter {
    pub fn write(statement: &TaxStatement, path: &str) -> EmptyResult {
        let mut writer = TaxStatementWriter {
            file: BufWriter::new(File::create(path)?),
            buffer: Rc::default(),
        };

        writer.write_raw(&get_header(statement.year))?;

        for record in &statement.records {
            record.write(&mut writer)?;
        }

        let footer = [0, 0];
        writer.write_bytes(&footer)?;

        Ok(())
    }

    pub fn write_value<T>(&mut self, value: &T) -> EmptyResult where T: TaxStatementType {
        TaxStatementType::write(value, self)
    }

    pub fn write_primitive<T>(&mut self, value: &T) -> EmptyResult where T: TaxStatementPrimitiveType {
        {
            let buffer = Rc::get_mut(&mut self.buffer).unwrap();
            buffer.clear();
            TaxStatementPrimitiveType::encode(value, buffer)?;
        }

        let buffer = Rc::clone(&self.buffer);
        Ok(self.write_data(&buffer)?)
    }

    pub fn write_data(&mut self, data: &str) -> EmptyResult {
        let encoded_data = encode(data)?;
        if encoded_data.len() > 9999 {
            return Err!("Unable to encode {:?}: Too big data size", data);
        }

        let size = format!("{:04}", encoded_data.len());
        assert_eq!(size.len(), 4);

        self.write_raw(&size)?;
        self.write_bytes(encoded_data.deref())?;

        Ok(())
    }

    fn write_raw(&mut self, data: &str) -> EmptyResult {
        self.write_bytes(&encode(data)?)
    }

    fn write_bytes(&mut self, data: &[u8]) -> EmptyResult {
        Ok(self.file.write_all(data)?)
    }
}

fn get_header(year: i32) -> String {
    format!(r"DLSG            Decl{}0102FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", year)
}

fn encode(data: &str) -> GenericResult<Cow<[u8]>> {
    let (encoded_data, _, errors) = encoding_rs::WINDOWS_1251.encode(data);
    if errors {
        return Err!("Unable to encode {:?} with Windows-1251 character encoding", data);
    }
    Ok(encoded_data)
}

tax_statement_record!(Nalog {
    unknown: Integer,
});

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
        let path = Path::new(file!()).parent().unwrap().join("testdata/filled.dc7")
            .to_str().unwrap().to_owned();

        let data = get_contents(&path);
        let mut statement = test_parsing(&path);
        let year = statement.year;

        let mut incomes = Vec::new();
        incomes.extend(statement.get_foreign_incomes().unwrap().unwrap().drain(..));
        assert_eq!(incomes.len(), 1);

        {
            let description = "Дивиденд";
            let date = date!(1, 1, year);
            let currency = "USD";
            let currency_rate = decf!(60.6569);
            let amount = dec!(100);
            let paid_tax = dec!(10);
            let local_amount = decf!(6065.69);
            let local_paid_tax = decf!(606.57);

            statement.add_dividend(
                description, date, currency, currency_rate,
                amount, paid_tax, local_amount, local_paid_tax).unwrap();
        }

        assert_eq!(*statement.get_foreign_incomes().unwrap().unwrap(), incomes);
        compare_to(&statement, &data);
    }

    #[test]
    fn parse_real() {
        test_parsing("testdata/statement.dc7");
    }

    fn test_parsing(path: &str) -> TaxStatement {
        let data = get_contents(path);

        let statement = TaxStatementReader::read(path).unwrap();
        assert_eq!(statement.year, 2017);
        compare_to(&statement, &data);

        statement
    }

    fn compare_to(statement: &TaxStatement, data: &str) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        TaxStatementWriter::write(&statement, path).unwrap();
        assert_eq!(&get_contents(path), data);
    }

    fn get_contents(path: &str) -> String {
        let mut data = vec![];

        File::open(path).unwrap().read_to_end(&mut data).unwrap();

        let (contents, _, errors) = encoding_rs::WINDOWS_1251.decode(data.as_slice());
        assert_eq!(errors, false);

        contents.deref().to_owned()
    }
}