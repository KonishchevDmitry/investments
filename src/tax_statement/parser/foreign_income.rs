use core::GenericResult;

use super::TaxStatementParser;
use super::record::{Record, ParseResult};

tax_statement_record!(CurrencyIncome {
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
});

#[derive(Debug)]
pub struct ForeignIncome {
    incomes: Vec<CurrencyIncome>,
}

impl ForeignIncome {
    pub fn parse(parser: &mut TaxStatementParser, name: String) -> ParseResult {
        let number: usize = parser.read_value()?;
        let mut incomes = Vec::with_capacity(number);

        for index in 0..number {
            {
                let name = parser.read_data()?;
                let expected_name = format!("@CurrencyIncome{:03}", index);

                if name != expected_name {
                    return Err!("Got {:?} where {:?} record is expected", name, expected_name);
                }
            }

            incomes.push(CurrencyIncome::parse(parser)?);
        }

        Ok((Box::new(ForeignIncome {incomes: incomes}), None))
    }
}

impl Record for ForeignIncome {
}