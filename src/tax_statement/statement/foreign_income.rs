use std::any::Any;

use core::{EmptyResult, GenericResult};
use types::{Date, Decimal};

use super::encoding::TaxStatementType;
use super::parser::{TaxStatementReader, TaxStatementWriter};
use super::record::Record;
use super::types::Integer;

tax_statement_array_record!(CurrencyIncome {
    type_: IncomeType,
    description: String,
    county_code: Integer,

    date: Date,
    tax_payment_date: Date,

    automatic_currency_convertion: bool,
    currency_code: Integer,
    currency_rate_for_income_date: Decimal,
    currency_rate_units_for_income_date: Integer,
    currency_rate_for_tax_payment_date: Decimal,
    currency_rate_units_for_tax_payment_date: Integer,
    currency_name: String,

    amount: Decimal,
    local_amount: Decimal,

    paid_tax: Decimal,
    local_paid_tax: Decimal,

    deduction_code: Integer,
    deduction_value: Decimal,

    unknown1: Integer,
    company_type: Integer,
    unknown2: String,
}, index_length=3);

#[derive(Debug)]
pub struct ForeignIncome {
    // FIXME: pub?
    pub incomes: Vec<CurrencyIncome>,
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
    fn name(&self) -> &str {
        ForeignIncome::RECORD_NAME
    }

    fn as_mut_any(&mut self) -> &mut Any {
        self
    }

    fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult {
        writer.write_data(ForeignIncome::RECORD_NAME)?;
        writer.write_value(&self.incomes.len())?;

        for (index, income) in self.incomes.iter().enumerate() {
            income.write(writer, index)?;
        }

        Ok(())
    }

}

#[derive(Debug, PartialEq, Clone)]
pub enum IncomeType {
    Dividend,
    Unknown {unknown: Integer, code: Integer, name: String},
}

impl IncomeType {
    fn decouple(&self) -> (Integer, Integer, String) {
        let (unknown, code, name) = match self {
            IncomeType::Dividend => (14, 1010, "Дивиденды"),
            IncomeType::Unknown {unknown, code, name} => return (*unknown, *code, name.clone()),
        };

        (unknown, code, name.to_owned())
    }
}

impl TaxStatementType for IncomeType {
    fn read(reader: &mut TaxStatementReader) -> GenericResult<IncomeType> {
        let unknown = reader.read_value()?;
        let code = reader.read_value()?;
        let name = reader.read_value()?;

        for income_type in [IncomeType::Dividend].iter() {
            let (other_unknown, other_code, other_name) = income_type.decouple();
            if unknown == other_unknown && code == other_code && name == other_name {
                return Ok(income_type.clone());
            }
        }

        Ok(IncomeType::Unknown {
            unknown: unknown,
            code: code,
            name: name,
        })
    }

    fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult {
        let (unknown, code, name) = self.decouple();
        writer.write_value(&unknown)?;
        writer.write_value(&code)?;
        writer.write_value(&name)?;
        Ok(())
    }
}