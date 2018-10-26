use core::{EmptyResult, GenericResult};

use self::foreign_income::{ForeignIncome, CurrencyIncome, IncomeType};
use self::record::Record;
use self::parser::TaxStatementReader;

#[macro_use] mod record;
mod encoding;
mod foreign_income;
mod parser;
mod types;

#[derive(Debug)]
pub struct TaxStatement {
    pub year: i32,
    records: Vec<Box<Record>>,
}

impl TaxStatement {
    pub fn read(path: &str) -> GenericResult<TaxStatement> {
        Ok(TaxStatementReader::read(path).map_err(|e| format!(
            "Error while reading {:?}: {}", path, e))?)
    }

    pub fn add_dividend(&mut self) -> EmptyResult {
        // FIXME
        let incomes = self.get_foreign_incomes()?.ok_or_else(|| format!(
            "Foreign income must be enabled in the tax statement to add dividend income"))?;

        incomes.push(CurrencyIncome {
            // FIXME: HERE
            income_type: IncomeType::Dividend,
            income_name: "".to_owned(),
            county_code: 1,
            income_date: date!(1, 1, 2000),
            tax_payment_date: date!(1, 1, 2000),
            automatic_currency_converting: true,
            currency_code: 1,
            currency_rate_for_income_date: "".to_owned(),  // FIXME: Decimal
            currency_rate_units_for_income_date: 1,
            currency_rate_for_tax_payment_date: "".to_owned(),  // FIXME: Decimal
            currency_rate_units_for_tax_payment_date: 1,
            currency_name: "".to_owned(),
            income_value: "".to_owned(),  // FIXME: Decimal
            income_value_in_local_currency: "".to_owned(),  // FIXME: Decimal
            paid_tax_value: "".to_owned(),  // FIXME: Decimal
            paid_tax_value_in_local_currency: "".to_owned(),  // FIXME: Decimal
            deduction_code: 1,
            deduction_value: "".to_owned(),  // FIXME: Decimal
            unknown2: "".to_owned(),
            company_type: "".to_owned(),
            unknown3: "".to_owned(),
        });

        Ok(())
    }

    fn get_foreign_incomes(&mut self) -> GenericResult<Option<&mut Vec<CurrencyIncome>>> {
        Ok(self.get_mut_record(ForeignIncome::RECORD_NAME)?
            .map(|record: &mut ForeignIncome| &mut record.incomes))
    }

    fn get_mut_record<T: 'static>(&mut self, name: &str) -> GenericResult<Option<&mut T>> {
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