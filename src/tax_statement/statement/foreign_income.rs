use core::{EmptyResult, GenericResult};
use types::Date;

use super::parser::{TaxStatementReader, TaxStatementWriter};
use super::record::Record;
use super::types::Integer;

tax_statement_array_record!(CurrencyIncome {
    // FIXME: HERE
    unknown1: Integer,
    income_type: Integer,
    income_type_name: String,
    income_name: String,
    county_code: Integer,
    income_date: Date,
    tax_payment_date: Date,
    automatic_currency_converting: bool,
    currency_code: Integer,
    currency_rate_for_income_date: String,  // FIXME: Decimal
    currency_rate_units_for_income_date: Integer,
    currency_rate_for_tax_payment_date: String,  // FIXME: Decimal
    currency_rate_units_for_tax_payment_date: Integer,
    currency_name: String,
    income_value: String,  // FIXME: Decimal
    income_value_in_local_currency: String,  // FIXME: Decimal
    paid_tax_value: String,  // FIXME: Decimal
    paid_tax_value_in_local_currency: String,  // FIXME: Decimal
    deduction_code: Integer,
    deduction_value: String,  // FIXME: Decimal
    unknown2: String,
    company_type: String,
    unknown3: String,
}, index_length=3);

#[derive(Debug)]
pub struct ForeignIncome {
    incomes: Vec<CurrencyIncome>,
}

impl ForeignIncome {
    pub const RECORD_NAME: &'static str = "@DeclForeign";

    pub fn read(reader: &mut TaxStatementReader) -> GenericResult<ForeignIncome> {
        let number: usize = reader.read_value()?;
        let mut incomes = Vec::with_capacity(number);

        for index in 0..number {
            incomes.push(CurrencyIncome::read(reader, index)?);
        }

        Ok(ForeignIncome {incomes: incomes})
    }
}

impl Record for ForeignIncome {
    fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult {
        writer.write_data(ForeignIncome::RECORD_NAME)?;
        writer.write_value(&self.incomes.len())?;

        for (index, income) in self.incomes.iter().enumerate() {
            income.write(writer, index)?;
        }

        Ok(())
    }

}