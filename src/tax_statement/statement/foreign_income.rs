use std::any::Any;

use crate::core::{EmptyResult, GenericResult};
use crate::currency;
use crate::types::{Date, Decimal};

use super::encoding::TaxStatementType;
use super::parser::{TaxStatementReader, TaxStatementWriter};
use super::record::Record;
use super::types::Integer;

#[derive(Debug)]
pub struct ForeignIncome {
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

    fn as_mut_any(&mut self) -> &mut dyn Any {
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

tax_statement_array_record!(CurrencyIncome {
    type_: IncomeType,
    description: String,
    county_code: CountryCode,

    date: Date,
    tax_payment_date: Date,
    currency: CurrencyInfo,

    amount: Decimal,
    local_amount: Decimal,

    paid_tax: Decimal,
    local_paid_tax: Decimal,
    deduction: DeductionInfo,

    controlled_foreign_company: ControlledForeignCompanyInfo,
}, index_length=3);

tax_statement_inner_record!(CurrencyInfo {
    automatic_convertion: bool,
    code: Integer,

    income_date_rate: Decimal,
    income_date_units: Integer,

    tax_payment_date_rate: Decimal,
    tax_payment_date_units: Integer,

    name: String,
});

impl CurrencyInfo {
    pub fn new(currency: &str, precise_currency_rate: Decimal) -> GenericResult<CurrencyInfo> {
        let (currency_code, currency_name, currency_rate_units) = match currency {
            "RUB" => (643, "Российский рубль", 1000),
            "USD" => (840, "Доллар сша", 100),
            _ => return Err!("{} currency is not supported yet", currency),
        };
        let currency_rate = currency::round(precise_currency_rate * Decimal::from(currency_rate_units));

        Ok(CurrencyInfo {
            automatic_convertion: true,
            code: currency_code,

            income_date_rate: currency_rate,
            income_date_units: currency_rate_units,

            tax_payment_date_rate: currency_rate,
            tax_payment_date_units: currency_rate_units,

            name: currency_name.to_owned(),
        })
    }
}

tax_statement_inner_record!(DeductionInfo {
    code: Integer,
    amount: Decimal,
});

impl DeductionInfo {
    pub fn new_none() -> DeductionInfo {
        DeductionInfo {
            code: 0,
            amount: dec!(0),
        }
    }
}

tax_statement_inner_record!(ControlledForeignCompanyInfo {
    unknown: Integer,
    profit_calculation_method: Integer,
    number: String,
    paid_tax: Integer,
});

impl ControlledForeignCompanyInfo {
    pub fn new_none() -> ControlledForeignCompanyInfo {
        ControlledForeignCompanyInfo {
            unknown: 0,
            profit_calculation_method: 0,
            number: String::new(),
            paid_tax: 0,
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum IncomeType {
    Dividend,
    Interest,
    Stock,
    Unknown {unknown: Integer, code: Integer, name: String},
}

impl IncomeType {
    fn decouple(&self) -> (Integer, Integer, String) {
        let (unknown, code, name) = match self {
            IncomeType::Dividend => (14, 1010, "Дивиденды"),
            IncomeType::Interest => (13, 1011, "Проценты (за исключением процентов по облигациям с ипотечным покрытием, эмитированным до 01.01.2007)"),
            IncomeType::Stock => (13, 1530, "(01)Доходы от реализации ЦБ (обращ-ся на орг. рынке ЦБ)"),
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

        for income_type in &[IncomeType::Dividend, IncomeType::Interest, IncomeType::Stock] {
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

#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub enum CountryCode {
    Usa,
    Unknown(Integer),
}

impl TaxStatementType for CountryCode {
    fn read(reader: &mut TaxStatementReader) -> GenericResult<CountryCode> {
        Ok(match reader.read_value()? {
            840 => CountryCode::Usa,
            code => CountryCode::Unknown(code),
        })
    }

    fn write(&self, writer: &mut TaxStatementWriter) -> EmptyResult {
        let code = match *self {
            CountryCode::Usa => 840,
            CountryCode::Unknown(code) => code,
        };
        writer.write_value(&code)?;
        Ok(())
    }
}