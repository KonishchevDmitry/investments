use std::borrow::Cow;
use std::fs::File;
use std::io::{Read, BufRead, BufReader, Write, BufWriter};
use std::ops::Deref;
#[cfg(test)] use std::path::Path;
use std::rc::Rc;

use chrono::Datelike;
use lazy_static::lazy_static;
use log::{trace, debug, warn};
use num_integer::Integer;
use regex::Regex;
#[cfg(test)] use tempfile::NamedTempFile;

use crate::core::{EmptyResult, GenericResult};
use crate::time;
#[cfg(test)] use crate::types::Decimal;
#[cfg(test)] use crate::util;

use super::TaxStatement;
#[cfg(test)] use super::countries::CountryCode;
use super::record::{Record, UnknownRecord, is_record_name};
use super::encoding::{TaxStatementType, TaxStatementPrimitiveType};
use super::foreign_income::ForeignIncome;

const SUPPORTED_YEAR: i32 = 2021;

pub struct TaxStatementReader {
    file: BufReader<File>,
    buffer: Vec<u8>,
}

impl TaxStatementReader {
    pub fn read(path: &str) -> GenericResult<TaxStatement> {
        lazy_static! {
            static ref EXTENSION_REGEX: Regex = Regex::new(r"\.dc(\d)$").unwrap();
        }

        let short_year = EXTENSION_REGEX.captures(path)
            .and_then(|captures| captures.get(1).unwrap().as_str().parse::<i32>().ok())
            .ok_or("Invalid tax statement file extension: *.dcX is expected")?;

        let (mut decade, current_short_year) = time::today().year().div_mod_floor(&10);
        if short_year > current_short_year + 1 {
            decade -= 1;
        }
        let year = decade * 10 + short_year;

        if year != SUPPORTED_YEAR {
            warn!(concat!(
                "Only *{} tax statements ({} year) are supported by the program. ",
                "Reading or writing tax statements for other years may have issues or won't work ",
                "at all."
            ), get_extension(SUPPORTED_YEAR), SUPPORTED_YEAR);
        }

        let mut reader = TaxStatementReader {
            file: BufReader::new(File::open(path)?),
            buffer: Vec::new(),
        };

        let header = get_header(year);
        if reader.read_raw(header.len())? != header {
            return Err!("The file has an unexpected header");
        }

        let mut records = Vec::new();
        let mut next_record_name = None;
        trace!("Parsing {:?} tax statement:", path);

        loop {
            let record_name = match next_record_name.take() {
                Some(record_name) => record_name,
                None => {
                    if reader.at_eof()? {
                        break;
                    }

                    let data: String = reader.read_value()?;
                    if !is_record_name(&data) {
                        return Err!("Got an invalid record name: {:?}", data);
                    }

                    data
                },
            };

            let record: Box<dyn Record> = match record_name.as_str() {
                ForeignIncome::RECORD_NAME => Box::new(ForeignIncome::read(&mut reader)?),
                _ => {
                    let (record, read_next_record_name) = UnknownRecord::read(&mut reader, record_name)?;
                    next_record_name = read_next_record_name;
                    Box::new(record)
                },
            };

            trace!("{:?}", record);
            records.push(record);
        }

        if records.is_empty() {
            return Err!("The tax statement has no records");
        }

        let statement = TaxStatement {
            path: path.to_owned(),
            year: year,
            records: records,
        };
        debug!("Read statement:\n{:#?}", statement);

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

    pub fn at_eof(&mut self) -> GenericResult<bool> {
        let mut buf = self.file.fill_buf()?;
        if buf.is_empty() {
            return Ok(true)
        }

        if buf[0] != 0 {
            return Ok(false);
        }

        // Декларация program sometimes writes zero bytes to the tail of the file. This behaviour
        // varies from version to version, seems to have no meaning and looks like a serialization
        // bug. So just ignore it.

        while !buf.is_empty() {
            if buf.iter().any(|&byte| byte != 0) {
                return Err!("Got an unexpected zero byte in the middle of the file");
            }

            let size = buf.len();
            self.file.consume(size);

            buf = self.file.fill_buf()?;
        }

        Ok(true)
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

    #[allow(clippy::rc_buffer)]
    buffer: Rc<String>,
}

impl TaxStatementWriter {
    pub fn write(statement: &TaxStatement, path: &str) -> EmptyResult {
        debug!("Statement to write:\n{:#?}", statement);

        let mut writer = TaxStatementWriter {
            file: BufWriter::new(File::create(path)?),
            buffer: Rc::default(),
        };

        writer.write_raw(&get_header(statement.year))?;

        for record in &statement.records {
            record.write(&mut writer)?;
        }

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
        self.write_data(&buffer)
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

fn get_extension(year: i32) -> String {
    format!(".dc{}", year % 10)
}

fn get_header(year: i32) -> String {
    format!(r"DLSG            Decl{}0103FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF", year)
}

fn encode(data: &str) -> GenericResult<Cow<[u8]>> {
    let (encoded_data, _, errors) = encoding_rs::WINDOWS_1251.encode(data);
    if errors {
        return Err!("Unable to encode {:?} with Windows-1251 character encoding", data);
    }
    Ok(encoded_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        let path = Path::new(file!()).parent().unwrap().join(get_path("empty"));
        test_parsing(path.to_str().unwrap());
    }

    #[test]
    fn parse_filled() {
        let path = Path::new(file!()).parent().unwrap().join(get_path("filled"))
            .to_str().unwrap().to_owned();

        let data = get_contents(&path);
        let mut statement = test_parsing(&path);
        let year = statement.year;

        let incomes: Vec<_> = statement.get_foreign_incomes().unwrap().drain(..).collect();
        assert!(!incomes.is_empty());

        let date = date!(year, 1, 1);
        let amount = dec!(100);
        let paid_tax = dec!(10);
        let purchase_local_cost = dec!(10);

        {
            let currency = "USD"; // 840 - Доллар США
            let currency_rate = dec!(73.8757);

            let local_amount = amount * currency_rate;
            let local_paid_tax = util::round(paid_tax * currency_rate, 2);

            // 840 - Код страны источника выплаты
            // 643 - Код страны зачисления выплаты
            // 1010 - Дивиденды
            statement.add_dividend_income(
                "Дивиденд", date, CountryCode::Usa, CountryCode::Russia,
                currency, currency_rate, amount, paid_tax, local_amount, local_paid_tax).unwrap();

            // 1530 - (01)Доходы от реализации ЦБ (обращ-ся на орг. рынке ЦБ)
            statement.add_stock_income(
                "Акции", date, CountryCode::Usa, currency, currency_rate, amount, local_amount,
                purchase_local_cost).unwrap();
        }

        struct CurrencyTestCase {
            name: &'static str,
            rate: Decimal,
        }

        for currency in [CurrencyTestCase {
            name: "EUR", // 978 - Евро
            rate: dec!(90.7932),
        }, CurrencyTestCase {
            name: "RUB", // 643 - Российский рубль
            rate: dec!(1),
        }, CurrencyTestCase {
            name: "GBP", // 826 - Фунт стерлингов
            rate: dec!(100.8477),
        }, CurrencyTestCase {
            name: "HKD", // 344 - Гонконгский доллар
            rate: dec!(9.53013),
        }, CurrencyTestCase {
            name: "AUD", // 036 - Австралийский доллар
            rate: dec!(56.9065),
        }] {
            let local_amount = crate::currency::round(amount * currency.rate);

            // 6013 - Доходы в виде процентов, полученных от источников за пределами Российской
            //        Федерации, в отношении которых применяется налоговая ставка, предусмотренная
            //        пунктом 1 статьи 224 Кодекса
            statement.add_interest_income(
                &format!("Проценты {}", currency.name), date, CountryCode::Usa,
                currency.name, currency.rate, amount, local_amount).unwrap();
        }

        for (expected, generated) in itertools::zip_eq(&incomes, statement.get_foreign_incomes().unwrap()) {
            assert_eq!(generated, expected);
        }
        compare_to(&statement, &data);
    }

    #[test]
    fn parse_real() {
        test_parsing(&get_path("statement"));
    }

    fn test_parsing(path: &str) -> TaxStatement {
        let data = get_contents(path);

        let statement = TaxStatementReader::read(path).unwrap();
        assert_eq!(statement.year, SUPPORTED_YEAR);
        compare_to(&statement, &data);

        statement
    }

    fn compare_to(statement: &TaxStatement, mut data: &str) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        // Декларация program sometimes writes zero bytes to the tail of the file. This behaviour
        // varies from version to version, seems to have no meaning and looks like a serialization
        // bug. So just ignore it.
        data = data.trim_end_matches('\0');
        assert!(!data.is_empty());

        TaxStatementWriter::write(statement, path).unwrap();
        assert_eq!(&get_contents(path), data);
    }

    fn get_path(name: &str) -> String {
        format!("testdata/{}{}", name, get_extension(SUPPORTED_YEAR))
    }

    fn get_contents(path: &str) -> String {
        let mut data = vec![];

        File::open(path).unwrap().read_to_end(&mut data).unwrap();

        let (contents, _, errors) = encoding_rs::WINDOWS_1251.decode(data.as_slice());
        assert!(!errors);

        contents.deref().to_owned()
    }
}