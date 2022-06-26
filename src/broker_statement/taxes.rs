use std::collections::HashMap;

use chrono::Datelike;

use crate::broker_statement::payments::{Payments, Withholding};
use crate::broker_statement::validators::DateValidator;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::formatting;
use crate::instruments::InstrumentId;
use crate::types::Date;

#[derive(PartialEq, Eq, Hash)]
pub struct TaxId {
    pub date: Date,
    pub issuer: InstrumentId,
}

impl TaxId {
    pub fn new(date: Date, issuer: InstrumentId) -> TaxId {
        TaxId {date, issuer}
    }

    pub fn description(&self) -> String {
        format!("{} tax withheld at {}", self.issuer, formatting::format_date(self.date))
    }
}

pub type TaxAccruals = Payments;

pub struct TaxAgentWithholdings {
    withholdings: Vec<TaxAgentWithholding>,
}

impl TaxAgentWithholdings {
    pub fn new() -> TaxAgentWithholdings {
        TaxAgentWithholdings {
            withholdings: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.withholdings.is_empty()
    }

    pub fn add(&mut self, date: Date, year: i32, amount: Withholding) -> EmptyResult {
        self.withholdings.push(TaxAgentWithholding::new(date, year, amount)?);
        Ok(())
    }

    pub fn merge(&mut self, other: TaxAgentWithholdings) {
        self.withholdings.extend(other.withholdings)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, TaxAgentWithholding> {
        self.withholdings.iter()
    }

    pub fn calculate(&self) -> GenericResult<HashMap<i32, Cash>> {
        let mut withholdings = HashMap::new();

        for tax in &self.withholdings {
            match tax.amount {
                Withholding::Withholding(amount) => {
                    withholdings.entry(tax.year)
                        .or_insert_with(|| Cash::zero(amount.currency))
                        .add_assign(amount).map_err(|e| format!(
                            "Got an unexpected tax withholding for {} year on {}: {}",
                            tax.year, formatting::format_date(tax.date), e,
                        ))?;
                },
                Withholding::Refund(amount) => {
                    let withheld = withholdings.entry(tax.year)
                        .or_insert_with(|| Cash::zero(amount.currency));

                    withheld.sub_assign(amount).map_err(|e| format!(
                        "Got an unexpected tax refund for {} year on {}: {}",
                        tax.year, formatting::format_date(tax.date), e,
                    ))?;

                    if withheld.is_negative() {
                        return Err!(
                            "Got an unexpected tax refund for {} year on {}: it's bigger then withheld amount by {}",
                            tax.year, formatting::format_date(tax.date), -*withheld);
                    }

                    if withheld.is_zero() {
                        withholdings.remove(&tax.year);
                    }
                },
            }
        }

        Ok(withholdings)
    }

    pub fn sort_and_validate(&mut self, validator: &DateValidator) -> EmptyResult {
        validator.sort_and_validate(
            "a tax agent withholding", &mut self.withholdings,
            |withholding| withholding.date)?;

        self.calculate()?;

        Ok(())
    }
}

impl<'a> IntoIterator for &'a TaxAgentWithholdings {
    type Item = &'a TaxAgentWithholding;
    type IntoIter = std::slice::Iter<'a, TaxAgentWithholding>;

    fn into_iter(self) -> std::slice::Iter<'a, TaxAgentWithholding> {
        self.iter()
    }
}

pub struct TaxAgentWithholding {
    pub date: Date,
    pub year: i32,
    pub amount: Withholding,
}

impl TaxAgentWithholding {
    pub fn new(date: Date, year: i32, amount: Withholding) -> GenericResult<TaxAgentWithholding> {
        if year != date.year() && year != date.year() - 1 {
            return Err!(
                "Got an unexpected {} year tax withholding on {}",
                year, formatting::format_date(date));
        }
        Ok(TaxAgentWithholding {date, year, amount: amount })
    }
}