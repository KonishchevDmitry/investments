use core::{EmptyResult, GenericResult};

use super::parser::{TaxStatementReader, TaxStatementWriter};
use super::record::Record;

tax_statement_array_record!(CurrencyIncome {
    /*
    class ForeignIncome(record_view("CurrencyIncome", (
    ("unknown1", Integer),
    ("income_type", Integer),
    ("income_type_name", String),
    ("income_name", String),
    ("county_code", Integer),
    ("income_date", Date),
    ("tax_payment_date", Date),
    ("automatic_currency_converting", Bool),
    ("currency_code", Integer),
    ("currency_rate_for_income_date", Currency),
    ("currency_rate_units_for_income_date", Integer),
    ("currency_rate_for_tax_payment_date", Currency),
    ("currency_rate_units_for_tax_payment_date", Integer),
    ("currency_name", String),
    ("income_value", Currency),
    ("income_value_in_local_currency", Currency),
    ("paid_tax_value", Currency),
    ("paid_tax_value_in_local_currency", Currency),
    ("deduction_code", Integer),
    ("deduction_value", Currency),
    ("unknown2", String),
    ("company_type", String),
    ("unknown3", String),
    */

    f01: String,
    f02: String,
    f03: String,
    f04: String,
    f05: String,
    f06: String,
    f07: String,
    f08: String,
    f09: String,
    f10: String,
    f11: String,
    f12: String,
    f13: String,
    f14: String,
    f15: String,
    f16: String,
    f17: String,
    f18: String,
    f19: String,
    f20: String,
    f21: String,
    f22: String,
    f23: String,
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