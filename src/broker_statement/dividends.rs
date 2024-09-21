use std::collections::HashMap;

use chrono::Datelike;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::instruments::{InstrumentId, IssuerTaxationType};
use crate::localities::Country;
use crate::taxes::{IncomeType, TaxCalculator, Tax};
use crate::time::Date;

use super::cash_flows::{CashFlow, CashFlowType};
use super::payments::Payments;
use super::taxes::{TaxId, TaxAccruals};

pub struct Dividend {
    pub date: Date,
    pub issuer: String,
    pub original_issuer: String,

    pub amount: Cash,
    pub paid_tax: Cash,
    pub taxation_type: IssuerTaxationType,
    pub skip_from_cash_flow: bool,
}

impl Dividend {
    pub fn tax(&self, country: &Country, converter: &CurrencyConverter, calculator: &mut TaxCalculator) -> GenericResult<Tax> {
        let amount = converter.convert_to_cash_rounding(self.date, self.amount, country.currency)?;

        Ok(match self.taxation_type {
            IssuerTaxationType::Manual{..} => {
                let paid_tax = converter.convert_to_cash_rounding(self.date, self.paid_tax, country.currency)?;
                calculator.tax_income(IncomeType::Dividends, self.date.year(), amount, Some(paid_tax))
            },
            IssuerTaxationType::TaxAgent{..} => {
                calculator.tax_agent_income(IncomeType::Dividends, self.date.year(), amount, self.paid_tax).map_err(|e| format!(
                    "{}: {}", self.description(), e))?
            },
        })
    }

    pub fn description(&self) -> String {
        format!("{} dividend from {}", self.original_issuer, formatting::format_date(self.date))
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct DividendId {
    pub date: Date,
    pub issuer: InstrumentId,
}

impl DividendId {
    pub fn new(date: Date, issuer: InstrumentId) -> DividendId {
        DividendId {date, issuer}
    }

    pub fn description(&self) -> String {
        format!("{} dividend from {}", self.issuer, formatting::format_date(self.date))
    }
}

pub type DividendAccruals = Payments;

pub fn process_dividend_accruals(
    dividend: DividendId, issuer: &str, taxation_type: IssuerTaxationType,
    accruals: DividendAccruals, taxes: &mut HashMap<TaxId, TaxAccruals>,
    cash_flow_details: bool,
) -> GenericResult<(Option<Dividend>, Vec<CashFlow>)> {
    let mut cash_flows = Vec::new();

    let (amount, dividend_transactions) = accruals.get_result().map_err(|e| format!(
        "Failed to process {} dividend from {}: {}",
        issuer, formatting::format_date(dividend.date), e
    ))?;

    let tax_id = TaxId::new(dividend.date, dividend.issuer.clone());
    let (paid_tax, tax_transactions) = taxes.remove(&tax_id).map_or_else(|| Ok((None, Vec::new())), |tax_accruals| {
        tax_accruals.get_result().map_err(|e| format!(
            "Failed to process {} tax from {}: {}",
            tax_id.issuer, formatting::format_date(tax_id.date), e))
    })?;

    if cash_flow_details {
        for transaction in dividend_transactions {
            cash_flows.push(CashFlow {
                date: transaction.date.into(),
                amount: transaction.cash,
                type_: CashFlowType::Dividend {
                    date: dividend.date,
                    issuer: issuer.to_owned(),
                },
            })
        }

        for transaction in tax_transactions {
            cash_flows.push(CashFlow {
                date: transaction.date.into(),
                amount: -transaction.cash,
                type_: CashFlowType::Tax {
                    date: dividend.date,
                    issuer: issuer.to_owned(),
                },
            })
        }
    }

    let dividend = match amount {
        Some(amount) => Some(Dividend {
            date: dividend.date,
            issuer: issuer.to_owned(),
            original_issuer: issuer.to_owned(),

            amount: amount,
            paid_tax: paid_tax.unwrap_or_else(|| Cash::zero(amount.currency)),
            taxation_type: taxation_type,
            skip_from_cash_flow: cash_flow_details,
        }),
        None => {
            if paid_tax.is_some() {
                return Err!("Got paid tax for reversed {} dividend from {}",
                            issuer, formatting::format_date(dividend.date));
            }
            None
        },
    };

    Ok((dividend, cash_flows))
}