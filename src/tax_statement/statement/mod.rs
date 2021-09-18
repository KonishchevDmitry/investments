#[macro_use] mod record;

mod countries;
mod encoding;
mod foreign_income;
mod parser;
mod types;

use std::fs;

use crate::core::{EmptyResult, GenericResult};
use crate::types::{Date, Decimal};

use self::foreign_income::{ForeignIncome, CurrencyIncome, CurrencyInfo, DeductionInfo, IncomeType,
                           ControlledForeignCompanyInfo};
use self::record::Record;
use self::parser::{TaxStatementReader, TaxStatementWriter};

pub use self::countries::CountryCode;

#[derive(Debug)]
pub struct TaxStatement {
    path: String,
    pub year: i32,
    records: Vec<Box<dyn Record>>,
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

    pub fn add_dividend_income(
        &mut self, description: &str, date: Date,
        country: CountryCode, currency: &str, currency_rate: Decimal,
        amount: Decimal, paid_tax: Decimal, local_amount: Decimal, local_paid_tax: Decimal,
    ) -> EmptyResult {
        self.get_foreign_incomes()?.push(CurrencyIncome {
            type_: IncomeType::Dividend,
            description: description.to_owned(),
            county_code: country,

            date: date,
            tax_payment_date: date,
            currency: CurrencyInfo::new(currency, currency_rate)?,

            amount: amount,
            local_amount: local_amount,

            paid_tax: paid_tax,
            local_paid_tax: local_paid_tax,
            deduction: DeductionInfo::new_none(),

            controlled_foreign_company: ControlledForeignCompanyInfo::new_none(),
        });

        Ok(())
    }

    pub fn add_interest_income(
        &mut self, description: &str, date: Date, currency: &str, currency_rate: Decimal,
        amount: Decimal, local_amount: Decimal,
    ) -> EmptyResult {
        self.get_foreign_incomes()?.push(CurrencyIncome {
            type_: IncomeType::Interest,
            description: description.to_owned(),
            county_code: CountryCode::Usa,

            date: date,
            tax_payment_date: date,
            currency: CurrencyInfo::new(currency, currency_rate)?,

            amount: amount,
            local_amount: local_amount,

            paid_tax: dec!(0),
            local_paid_tax: dec!(0),
            deduction: DeductionInfo::new_none(),

            controlled_foreign_company: ControlledForeignCompanyInfo::new_none(),
        });

        Ok(())
    }

    pub fn add_stock_income(
        &mut self, description: &str, date: Date, currency: &str, currency_rate: Decimal,
        amount: Decimal, local_amount: Decimal, purchase_local_cost: Decimal,
    ) -> EmptyResult {
        self.get_foreign_incomes()?.push(CurrencyIncome {
            type_: IncomeType::Stock,
            description: description.to_owned(),
            county_code: CountryCode::Usa,

            date: date,
            tax_payment_date: date,
            currency: CurrencyInfo::new(currency, currency_rate)?,

            amount: amount,
            local_amount: local_amount,

            paid_tax: dec!(0),
            local_paid_tax: dec!(0),

            // Please note that we should always specify this deduction amount - even if it's zero.
            // If it's not specified the income doesn't participate into settlement of losses.
            deduction: DeductionInfo {
                code: 201,
                amount: purchase_local_cost,
            },

            controlled_foreign_company: ControlledForeignCompanyInfo::new_none(),
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