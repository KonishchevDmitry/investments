use std::collections::HashMap;

use chrono::Datelike;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::instruments::IssuerTaxationType;
use crate::localities::Country;
use crate::taxes::IncomeType;
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
    pub fn tax(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Cash> {
        Ok(match self.taxation_type {
            IssuerTaxationType::Manual => {
                let amount = converter.convert_to_cash_rounding(self.date, self.amount, country.currency)?;
                country.tax_to_pay(IncomeType::Dividends, self.date.year(), amount, None)
            },
            IssuerTaxationType::TaxAgent => {
                if self.paid_tax.currency != country.currency {
                    return Err!(
                        "Got withheld tax for {} in an unexpected currency: {}",
                        self.description(), self.paid_tax.currency)
                }
                self.paid_tax
            },
        })
    }

    pub fn tax_to_pay(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Cash> {
        Ok(match self.taxation_type {
            IssuerTaxationType::Manual => {
                let amount = converter.convert_to_cash_rounding(self.date, self.amount, country.currency)?;
                let paid_tax = converter.convert_to_cash_rounding(self.date, self.paid_tax, country.currency)?;
                country.tax_to_pay(IncomeType::Dividends, self.date.year(), amount, Some(paid_tax))
            },
            IssuerTaxationType::TaxAgent => {
                Cash::zero(country.currency)
            },
        })
    }

    pub fn description(&self) -> String {
        format!("{} dividend from {}", self.original_issuer, formatting::format_date(self.date))
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct DividendId {
    pub date: Date,
    pub issuer: String,
}

impl DividendId {
    pub fn new(date: Date, issuer: &str) -> DividendId {
        DividendId {date, issuer: issuer.to_owned()}
    }
}

pub type DividendAccruals = Payments;

pub fn process_dividend_accruals(
    dividend: DividendId, taxation_type: IssuerTaxationType,
    accruals: DividendAccruals, taxes: &mut HashMap<TaxId, TaxAccruals>,
    cash_flow_details: bool,
) -> GenericResult<(Option<Dividend>, Vec<CashFlow>)> {
    let mut cash_flows = Vec::new();

    let (amount, dividend_transactions) = accruals.get_result().map_err(|e| format!(
        "Failed to process {} dividend from {}: {}",
        dividend.issuer, formatting::format_date(dividend.date), e
    ))?;

    let tax_id = TaxId::new(dividend.date, &dividend.issuer);
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
                    issuer: dividend.issuer.clone(),
                },
            })
        }

        for transaction in tax_transactions {
            cash_flows.push(CashFlow {
                date: transaction.date.into(),
                amount: -transaction.cash,
                type_: CashFlowType::Tax {
                    date: dividend.date,
                    issuer: dividend.issuer.clone(),
                },
            })
        }
    }

    let dividend = match amount {
        Some(amount) => Some(Dividend {
            date: dividend.date,
            issuer: dividend.issuer.clone(),
            original_issuer: dividend.issuer,

            amount: amount,
            paid_tax: paid_tax.unwrap_or_else(|| Cash::zero(amount.currency)),
            taxation_type: taxation_type,
            skip_from_cash_flow: cash_flow_details,
        }),
        None => {
            if paid_tax.is_some() {
                return Err!("Got paid tax for reversed {} dividend from {}",
                            dividend.issuer, formatting::format_date(dividend.date));
            }
            None
        },
    };

    Ok((dividend, cash_flows))
}