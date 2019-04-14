use std::fs;

use crate::core::{EmptyResult, GenericResult};
use crate::types::{Date, Decimal};

use self::foreign_income::{ForeignIncome, CurrencyIncome, CurrencyInfo, IncomeType,
                           ControlledForeignCompanyInfo};
use self::record::Record;
use self::parser::{TaxStatementReader, TaxStatementWriter};

#[macro_use] mod record;
mod encoding;
mod foreign_income;
mod parser;
mod types;

#[derive(Debug)]
pub struct TaxStatement {
    path: String,
    pub year: i32,
    records: Vec<Box<Record>>,
}

impl TaxStatement {
    pub fn read(path: &str) -> GenericResult<TaxStatement> {
        Ok(TaxStatementReader::read(path).map_err(|e| format!(
            "Error while reading {:?} tax statement: {}", path, e))?)
    }

    pub fn save(&self) -> EmptyResult {
        let temp_path = format!("{}.new", self.path);

        TaxStatementWriter::write(self, &temp_path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            format!("Failed to save the tax statement to {:?}: {}", temp_path, e)
        })?;

        fs::rename(&temp_path, &self.path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            format!("Failed to rename {:?} to {:?}: {}", temp_path, self.path, e)
        })?;

        Ok(())
    }

    pub fn add_dividend(
        &mut self, description: &str, date: Date, currency: &str, currency_rate: Decimal,
        amount: Decimal, paid_tax: Decimal, local_amount: Decimal, local_paid_tax: Decimal,
    ) -> EmptyResult {
        let (country_code, currency_info) = CurrencyInfo::new(currency, currency_rate)?;

        self.get_foreign_incomes()?.push(CurrencyIncome {
            type_: IncomeType::Dividend,
            description: description.to_owned(),
            county_code: country_code,

            date: date,
            tax_payment_date: date,
            currency: currency_info,

            amount: amount,
            local_amount: local_amount,

            paid_tax: paid_tax,
            local_paid_tax: local_paid_tax,

            deduction_code: 0,
            deduction_value: dec!(0),

            controlled_foreign_company: ControlledForeignCompanyInfo::new_empty(),
        });

        Ok(())
    }

    pub fn add_interest(
        &mut self, description: &str, date: Date, currency: &str, currency_rate: Decimal,
        amount: Decimal, local_amount: Decimal,
    ) -> EmptyResult {
        let (country_code, currency_info) = CurrencyInfo::new(currency, currency_rate)?;

        self.get_foreign_incomes()?.push(CurrencyIncome {
            type_: IncomeType::Interest,
            description: description.to_owned(),
            county_code: country_code,

            date: date,
            tax_payment_date: date,
            currency: currency_info,

            amount: amount,
            local_amount: local_amount,

            paid_tax: dec!(0),
            local_paid_tax: dec!(0),

            deduction_code: 0,
            deduction_value: dec!(0),

            controlled_foreign_company: ControlledForeignCompanyInfo::new_empty(),
        });

        Ok(())
    }

    fn get_foreign_incomes(&mut self) -> GenericResult<&mut Vec<CurrencyIncome>> {
        Ok(self.get_mut_record(ForeignIncome::RECORD_NAME)?
            .map(|record: &mut ForeignIncome| &mut record.incomes)
            .ok_or("Foreign income must be enabled in the tax statement")?)
    }

    fn get_mut_record<T: 'static + Record>(&mut self, name: &str) -> GenericResult<Option<&mut T>> {
        let mut found_record = None;

        for record in &mut self.records {
            if record.name() != name {
                continue;
            }

            if found_record.is_some() {
                return Err!("The statement has several {} records", name);
            }

            found_record = Some(record);
        }

        Ok(match found_record {
            Some(record) => Some(
                record.as_mut_any().downcast_mut::<T>().ok_or_else(|| format!(
                    "Failed to cast {} record to the underlaying type", name))?),
            None => None,
        })
    }
}