mod countries;
mod encoding;
mod foreign_income;

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use chrono::Datelike;
use log::{trace, warn};
use num_integer::Integer;
use serde_json::{Map, Value};
#[cfg(test)] use tempfile::NamedTempFile;

use crate::core::{EmptyResult, GenericError, GenericResult};
use crate::time;
use crate::types::{Date, Decimal};
use crate::util;

pub use self::countries::CountryCode;
use self::foreign_income::{Currency, Deduction, ForeignIncome, IncomeType, PaidTax};

const SUPPORTED_YEAR: i32 = 2025;

#[derive(Debug)]
pub struct TaxStatement {
    path: PathBuf,
    pub year: i32,
    contents: Map<String, Value>,
    pub new_income_added: bool,
}

impl TaxStatement {
    pub fn read(path: &Path, year_hint: Option<i32>) -> GenericResult<TaxStatement> {
        let short_year: i32 = path.extension().and_then(OsStr::to_str).and_then(|extension| {
            extension.strip_prefix("de")?.parse::<u8>().ok()
        }).ok_or("Invalid tax statement file extension: *.deX is expected")?.into();

        let mut current_year = time::today().year();
        // In regression tests we work with ancient (often fake) tax statements and we have to properly guess the decade
        if let Some(year_hint) = year_hint && cfg!(debug_assertions) {
            current_year = year_hint;
        }

        let (mut decade, current_short_year) = current_year.div_mod_floor(&10);
        if short_year > current_short_year + 1 {
            decade -= 1;
        }
        let year = decade * 10 + short_year;

        if year != SUPPORTED_YEAR {
            warn!(concat!(
                "Only *{} tax statements ({} year) are supported by the program. ",
                "Reading or writing tax statements for other years may have issues or won't work at all."
            ), get_extension(SUPPORTED_YEAR), SUPPORTED_YEAR);
        }

        let mut file = BufReader::new(File::open(path)?);

        let contents = match serde_json::from_reader(&mut file)? {
            Value::Object(object) => object,
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
                return Err!("The file contents is not a JSON object")
            },
        };

        let statement = TaxStatement {
            path: path.to_owned(),
            year,
            contents,
            new_income_added: false,
        };

        // Basic validation
        for key in [Self::ENABLED_SECTIONS_KEY, Self::FOREIGN_INCOME_KEY, Self::FOREIGN_INCOME_QUANTITY_KEY] {
            statement.get(key)?;
        }

        Ok(statement)
    }

    pub fn save(&self) -> EmptyResult {
        let temp_path = util::temp_path(&self.path);

        File::create(&temp_path).map_err(GenericError::from).and_then(|file| {
            let writer = BufWriter::new(file);
            Ok(serde_json::to_writer_pretty(writer, &self.contents)?)
        }).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            format!("Failed to save the tax statement to {temp_path:?}: {e}")
        })?;

        fs::rename(&temp_path, &self.path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            format!("Failed to rename {temp_path:?} to {:?}: {e}", self.path)
        })?;

        Ok(())
    }

    pub fn add_dividend_income(
        &mut self, description: &str, date: Date,
        source_from: CountryCode, received_in: CountryCode, currency: &str, currency_rate: Decimal,
        amount: Decimal, paid_tax: Decimal, local_amount: Decimal, local_paid_tax: Decimal,
    ) -> EmptyResult {
        self.add_foreign_income(ForeignIncome {
            type_: IncomeType::Dividend,
            description: description.to_owned(),

            date,
            tax_payment_date: date,

            source_from,
            received_in,
            currency: Currency::new(currency, currency_rate)?,

            amount,
            local_amount,

            paid_tax: Some(PaidTax::new(paid_tax, local_paid_tax)),
            deduction: None,

            controlled_foreign_company: None,
        })
    }

    pub fn add_interest_income(
        &mut self, description: &str, date: Date, broker_jurisdiction: CountryCode,
        currency: &str, currency_rate: Decimal, amount: Decimal, local_amount: Decimal,
    ) -> EmptyResult {
        self.add_foreign_income(ForeignIncome {
            type_: IncomeType::Interest,
            description: description.to_owned(),

            date,
            tax_payment_date: date,

            source_from: broker_jurisdiction,
            received_in: broker_jurisdiction,
            currency: Currency::new(currency, currency_rate)?,

            amount,
            local_amount,

            paid_tax: None,
            deduction: None,

            controlled_foreign_company: None,
        })
    }

    pub fn add_stock_income(
        &mut self, description: &str, date: Date, broker_jurisdiction: CountryCode,
        currency: &str, currency_rate: Decimal, amount: Decimal, local_amount: Decimal,
        purchase_local_cost: Decimal,
    ) -> EmptyResult {
        self.add_foreign_income(ForeignIncome {
            type_: IncomeType::Stock,
            description: description.to_owned(),

            date,
            tax_payment_date: date,

            source_from: broker_jurisdiction,
            received_in: broker_jurisdiction,
            currency: Currency::new(currency, currency_rate)?,

            amount,
            local_amount,

            paid_tax: None,
            // Please note that we should always specify this deduction amount - even if it's zero.
            // If it's not specified the income doesn't participate into settlement of losses.
            deduction: Some(Deduction {
                code: "201", // Расходы по операциям с ЦБ (обращ-ся на орг. рынке ЦБ)
                amount: purchase_local_cost,
            }),

            controlled_foreign_company: None,
        })
    }

    fn add_foreign_income(&mut self, income: ForeignIncome) -> EmptyResult {
        trace!("Adding the following income to the tax statement:\n{income:#?}");

        let value = serde_json::to_value(income).map_err(|e| format!(
            "Failed to encode foreign income tax statement record: {e}"))?;

        Ok(self.add_foreign_income_inner(value).map_err(|e| format!(
            "{:?} has an unexpected contents: {e}", self. path))?)
    }

    const FOREIGN_INCOME_KEY: &str = "CurrencyIncomeList";
    const FOREIGN_INCOME_QUANTITY_KEY: &str = "CurrencyQuantity";

    fn add_foreign_income_inner(&mut self, income: Value) -> EmptyResult {
        self.enable_foreign_income()?;

        {
            self.get_mut(Self::FOREIGN_INCOME_KEY)?.as_array_mut().ok_or_else(|| format!(
                "{:?} has an unexpected type", Self::FOREIGN_INCOME_KEY)
            )?.push(income);

            self.new_income_added = true;
        }

        {
            let quantity_value = self.get_mut(Self::FOREIGN_INCOME_QUANTITY_KEY)?;

            let new_quantity = quantity_value.as_u64().ok_or_else(|| format!(
                "{:?} has an unexpected value: {quantity_value}",
                Self::FOREIGN_INCOME_QUANTITY_KEY,
            ))? + 1;

            *quantity_value = Value::Number(new_quantity.into());
        }

        Ok(())
    }

    const ENABLED_SECTIONS_KEY: &str = "ContentSet";

    fn enable_foreign_income(&mut self) -> EmptyResult {
        let enabled_sections_decimal_bitmask_value = self.get_mut(Self::ENABLED_SECTIONS_KEY)?;

        let mut enabled_sections_bitmask = enabled_sections_decimal_bitmask_value.as_u64().and_then(|decimal_bitmask| {
            let bitmask_string = decimal_bitmask.to_string();
            u8::from_str_radix(&bitmask_string, 2).ok()
        }).ok_or_else(|| format!(
            "{:?} has an unexpected value: {enabled_sections_decimal_bitmask_value}",
            Self::ENABLED_SECTIONS_KEY,
        ))?;

        let foreign_income_flag = 1 << 1;
        if enabled_sections_bitmask & foreign_income_flag == 0 {
            trace!("Foreign income is not enabled in the tax statement. Enabling it.");
            enabled_sections_bitmask |= foreign_income_flag;

            let enabled_sections_decimal_bitmask: u64 = format!("{enabled_sections_bitmask:b}").parse().unwrap();
            *enabled_sections_decimal_bitmask_value = Value::Number(enabled_sections_decimal_bitmask.into());
        }

        Ok(())
    }

    fn get(&self, name: &str) -> GenericResult<&Value> {
        Ok(self.contents.get(name).ok_or_else(|| format!(
            "{name:?} key is missing"))?)
    }

    fn get_mut(&mut self, name: &str) -> GenericResult<&mut Value> {
        Ok(self.contents.get_mut(name).ok_or_else(|| format!(
            "{name:?} key is missing"))?)
    }
}

fn get_extension(year: i32) -> String {
    format!(".de{}", year % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filled() {
        let mut statement = TaxStatement::read(&get_path("empty"), None).unwrap();
        assert_eq!(statement.year, SUPPORTED_YEAR);

        let date = date!(statement.year, 1, 1);
        let amount = dec!(100);
        let paid_tax = dec!(10);
        let purchase_local_cost = dec!(10);

        {
            let currency = "USD"; // 840 - Доллар США
            let currency_rate = dec!(101.6797);

            let local_amount = amount * currency_rate;
            let local_paid_tax = util::round(paid_tax * currency_rate, 2);

            // 840 (Соединённые Штаты Америки) - Код страны источника выплаты
            // 840 (Соединённые Штаты Америки) - Код страны зачисления выплаты
            // 1530 - (01)Доходы от реализации ЦБ (обращ-ся на орг. рынке ЦБ)
            statement.add_stock_income(
                "Акции", date, CountryCode::Usa, currency, currency_rate, amount, local_amount,
                purchase_local_cost).unwrap();

            // 840 - Код страны источника выплаты
            // 643 - Код страны зачисления выплаты
            // 1010 - Дивиденды
            statement.add_dividend_income(
                "Дивиденд", date, CountryCode::Usa, CountryCode::Russia,
                currency, currency_rate, amount, paid_tax, local_amount, local_paid_tax).unwrap();
        }

        struct CurrencyTestCase {
            name: &'static str,
            rate: Decimal,
        }

        for currency in [CurrencyTestCase {
            name: "AUD", // 036 - Австралийский доллар
            rate: dec!(63.1533),
        }, CurrencyTestCase {
            name: "EUR", // 978 - Евро
            rate: dec!(106.1028),
        }, CurrencyTestCase {
            name: "GBP", // 826 - Фунт стерлингов
            rate: dec!(127.4962),
        }, CurrencyTestCase {
            name: "HKD", // 344 - Гонконгский доллар
            rate: dec!(13.1225),
        }, CurrencyTestCase {
            name: "RUB", // 643 - Российский рубль
            rate: dec!(1),
        }] {
            let local_amount = crate::currency::round(amount * currency.rate);

            // 6013 - Доходы в виде процентов, полученных от источников за пределами Российской
            //        Федерации, в отношении которых применяется налоговая ставка, предусмотренная
            //        пунктом 1 статьи 224 Кодекса
            statement.add_interest_income(
                &format!("Проценты {}", currency.name), date, CountryCode::Usa, currency.name, currency.rate,
                amount, local_amount).unwrap();
        }

        let temp_file = NamedTempFile::new().unwrap();
        statement.path = temp_file.path().to_owned();
        statement.save().unwrap();

        assert_statements(temp_file.path(), &get_path("filled"));
    }

    fn assert_statements(result_path: &Path, expected_path: &Path) {
        let result = get_contents(result_path);
        let expected = get_contents(expected_path);

        if result != expected {
            pretty_assertions::assert_eq!(
                serde_json::to_string_pretty(&result).unwrap(),
                serde_json::to_string_pretty(&expected).unwrap(),
            );
            assert_eq!(result, expected);
        }
    }

    fn get_path(name: &str) -> PathBuf {
        let relative_path = format!("testdata/{name}{}", get_extension(SUPPORTED_YEAR));
        Path::new(file!()).parent().unwrap().join(relative_path)
    }

    fn get_contents(path: &Path) -> Value {
        let mut reader = BufReader::new(File::open(path).unwrap());
        serde_json::from_reader(&mut reader).unwrap()
    }
}