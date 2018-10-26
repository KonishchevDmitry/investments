use core::{EmptyResult, GenericResult};
use types::{Date, Decimal};

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

    pub fn add_dividend(
        &mut self, description: &str, date: Date, currency: &str, currency_rate: Decimal,
        amount: Decimal, local_amount: Decimal, paid_tax: Decimal, tax_to_pay: Decimal,
    ) -> EmptyResult {
        let (country_code, currency_code, currency_name) = match currency {
            "USD" => (840, 840, "Доллар сша"),
            _ => return Err!("{} currency is not supported yet", currency),
        };

        let currency_rate_units = 100;
        let currency_rate = currency_rate * Decimal::from(currency_rate_units);

        // FIXME
        let incomes = self.get_foreign_incomes()?.ok_or_else(|| format!(
            "Foreign income must be enabled in the tax statement to add dividend income"))?;

        incomes.push(CurrencyIncome {
            type_: IncomeType::Dividend,
            description: description.to_owned(),
            county_code: country_code,

            date: date,
            tax_payment_date: date,

            automatic_currency_convertion: true,
            currency_code: currency_code,
            currency_rate_for_income_date: currency_rate,
            currency_rate_units_for_income_date: currency_rate_units,
            currency_rate_for_tax_payment_date: currency_rate,
            currency_rate_units_for_tax_payment_date: currency_rate_units,
            currency_name: currency_name.to_owned(),

            amount: amount,
            local_amount: local_amount,

            paid_tax: paid_tax,
            tax_to_pay: tax_to_pay,

            deduction_code: 0,
            deduction_value: dec!(0),

            unknown1: 0,
            company_type: 0,
            unknown2: String::new(),
        });

        Ok(())
    }

    fn get_foreign_incomes(&mut self) -> GenericResult<Option<&mut Vec<CurrencyIncome>>> {
        Ok(self.get_mut_record(ForeignIncome::RECORD_NAME)?
            .map(|record: &mut ForeignIncome| &mut record.incomes))
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