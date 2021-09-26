use std::str::FromStr;

use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;

use crate::broker_statement::open::common::deserialize_date;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::formatting;
use crate::instruments::ISIN_REGEX;
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

#[derive(Deserialize)]
pub struct CashFlows {
    #[serde(rename = "item")]
    cash_flows: Vec<CashFlow>,
}

impl CashFlows {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for cash_flow in &self.cash_flows {
            cash_flow.parse(statement)?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct CashFlow {
    #[serde(rename = "operationdate", deserialize_with = "deserialize_date")]
    date: Date,
    #[serde(rename = "analyticname")]
    operation: String,
    #[serde(rename = "comment")]
    description: String,
    #[serde(rename = "currencycode")]
    currency: String,
    #[serde(rename = "amount")]
    amount: Decimal,
}

impl CashFlow {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let date = self.date;
        let currency = &self.currency;
        let amount = self.amount;

        match self.operation.as_str() {
            "Перевод между площадками/счетами ДС" => {
                let amount = util::validate_named_cash(
                    "deposit or withdrawal amount", currency, amount,
                    DecimalRestrictions::NonZero)?;

                statement.deposits_and_withdrawals.push(CashAssets::new_from_cash(date, amount));
            },

            "Комиссия Брокера за заключение сделок" => {
                // It's taken into account during trades processing
            },

            "Зачисление дивидендов" => {
                let (isin, paid_tax) = parse_dividend_description(&self.description)?;

                let issuer_id = isin; // FIXME(konishchev): Implement
                let mut amount = util::validate_named_cash(
                    "dividend amount", currency, amount,
                    DecimalRestrictions::StrictlyPositive)?;

                if let Some(paid_tax) = paid_tax {
                    if paid_tax.currency != amount.currency {
                        return Err!(
                            "Got paid tax for {} dividend ({}) in an unexpected currency: {} vs {}",
                            isin, formatting::format_date(date), paid_tax.currency, amount.currency);
                    }
                    amount += paid_tax;
                    statement.tax_accruals(date, issuer_id, true).add(date, paid_tax);
                }

                statement.dividend_accruals(date, issuer_id, true).add(date, amount);
            },

            _ => return Err!("Unsupported cash flow type: {:?}", self.operation),
        }

        Ok(())
    }
}

fn parse_dividend_description(description: &str) -> GenericResult<(&str, Option<Cash>)> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(&format!(concat!(
            r"^Начисление дивидендов: количество {quantity}, ",
            r"ставка {amount} (?P<currency>{currency}), ",
            r"удержан налог эмитентом (?P<paid_tax>{amount}), ",
            r"{issuer_type}, {issuer_name}, (?P<isin>{isin}), дата среза {date}$",
        ), quantity=r"\d+", amount=r"(:?0|[1-9][0-9]*)(:?\.[0-9]+)?", currency=r"[A-Z]{3}",
           issuer_type=r"[^,]+", issuer_name=r"[^,]+", isin=ISIN_REGEX,
           date=r"\d{2}\.\d{2}\.\d{4}"),
        ).unwrap();
    }

    let (isin, currency, paid_tax) = REGEX.captures(description).and_then(|captures| {
        let isin = captures.name("isin").unwrap().as_str();
        let currency = captures.name("currency").unwrap().as_str();

        let paid_tax = match Decimal::from_str(captures.name("paid_tax").unwrap().as_str()) {
            Ok(amount) => amount,
            Err(_) => return None,
        };

        Some((isin, currency, paid_tax))
    }).ok_or_else(|| format!("Unsupported dividend description: {:?}", description))?;

    Ok((isin, Some(Cash::new(currency, paid_tax))))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(description, isin, paid_tax,
        case(concat!(
            "Начисление дивидендов: количество 1, ставка 0.42 USD, удержан налог эмитентом 0.04, ",
            "АО, The Coca-Cola Company, US1912161007, дата среза 15.06.2021"
        ), "US1912161007", Some(Cash::new("USD", dec!(0.04)))),
    )]
    fn dividend_description_parsing(description: &str, isin: &str, paid_tax: Option<Cash>) {
        assert_eq!(parse_dividend_description(description).unwrap(), (isin, paid_tax));
    }
}