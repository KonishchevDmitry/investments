use std::str::FromStr;

use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;

use crate::broker_statement::open::common::deserialize_date;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::EmptyResult;
use crate::currency::{Cash, CashAssets};
use crate::formatting;
use crate::instruments::{InstrumentId, ISIN_REGEX};
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
                let (issuer_id, paid_tax) = parse_dividend_description(&self.description).ok_or_else(|| format!(
                    "Unsupported dividend description: {:?}", self.description))?;

                let mut amount = util::validate_named_cash(
                    "dividend amount", currency, amount,
                    DecimalRestrictions::StrictlyPositive)?;

                if let Some(paid_tax) = paid_tax {
                    if paid_tax.currency != amount.currency {
                        return Err!(
                            "Got paid tax for {} dividend ({}) in an unexpected currency: {} vs {}",
                            issuer_id, formatting::format_date(date), paid_tax.currency, amount.currency);
                    }
                    amount += paid_tax;
                    statement.tax_accruals(date, issuer_id.clone(), true).add(date, paid_tax);
                }

                statement.dividend_accruals(date, issuer_id, true).add(date, amount);
            },

            _ => return Err!("Unsupported cash flow type: {:?}", self.operation),
        }

        Ok(())
    }
}

fn parse_dividend_description(description: &str) -> Option<(InstrumentId, Option<Cash>)> {
    const AMOUNT_REGEX: &str = r"(?:0|[1-9][0-9]*)(?:\.[0-9]+)?";

    lazy_static! {
        static ref ORDINARY_DIVIDEND_REGEX: Regex = Regex::new(&format!(concat!(
            r"^Начисление дивидендов: количество {quantity}, ",
            r"ставка {amount} (?P<currency>{currency}), ",
            r"удержан налог эмитентом (?P<paid_tax>{amount}), ",
            r"{issuer_type}, {issuer_name}, (?P<isin>{isin}), дата среза {date}$",
        ), quantity=r"\d+", amount=AMOUNT_REGEX, currency=r"[A-Z]{3}",
           issuer_type=r"[^,]+", issuer_name=r"[^,]+(?:, [^,]+)?", isin=ISIN_REGEX,
           date=r"\d{2}\.\d{2}\.\d{4}"),
        ).unwrap();

        static ref DEPOSITARY_RECEIPT_REGEX: Regex = Regex::new(&format!(concat!(
            r"^Выплата дохода клиент <{account}> дивиденды <(?P<issuer>{issuer})>",
            r"(?:, комиссия платежного агента <{commission}> {currency})?$",
        ), account=r"[^>]+", issuer=r"[^>]+", commission=AMOUNT_REGEX, currency=r"[^,]+")).unwrap();
    }

    if let Some(captures) = ORDINARY_DIVIDEND_REGEX.captures(description) {
        let isin = captures.name("isin").unwrap().as_str();
        let currency = captures.name("currency").unwrap().as_str();
        let paid_tax = match Decimal::from_str(captures.name("paid_tax").unwrap().as_str()) {
            Ok(paid_tax) => paid_tax,
            Err(_) => return None,
        };
        Some((
            InstrumentId::Isin(isin.to_owned()),
            Some(Cash::new(currency, paid_tax)),
        ))
    } else if let Some(captures) = DEPOSITARY_RECEIPT_REGEX.captures(description) {
        let issuer = captures.name("issuer").unwrap().as_str();
        Some((InstrumentId::InternalId(issuer.to_owned()), None))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(description, issuer_id, paid_tax,
        case(concat!(
            "Начисление дивидендов: количество 1, ставка 0.42 USD, удержан налог эмитентом 0.04, ",
            "АО, The Coca-Cola Company, US1912161007, дата среза 15.06.2021"
        ), InstrumentId::Isin(s!("US1912161007")), Some(Cash::new("USD", dec!(0.04)))),

        case(concat!(
            "Начисление дивидендов: количество 25, ставка 0.86 USD, удержан налог эмитентом 2.15, ",
            "АО, Altria Group, Inc., US02209S1033, дата среза 15.06.2021"
        ), InstrumentId::Isin(s!("US02209S1033")), Some(Cash::new("USD", dec!(2.15)))),

        case(concat!(
            "Выплата дохода клиент <123456> дивиденды <ABBVIE INC-ао>",
        ), InstrumentId::InternalId(s!("ABBVIE INC-ао")), None),

        case(concat!(
            "Выплата дохода клиент <123456> дивиденды <BRITISH AMERN TOB PLC-ADR>, ",
            "комиссия платежного агента <0.20> долларов",
        ), InstrumentId::InternalId(s!("BRITISH AMERN TOB PLC-ADR")), None),
    )]
    fn dividend_description_parsing(description: &str, issuer_id: InstrumentId, paid_tax: Option<Cash>) {
        assert_eq!(parse_dividend_description(description), Some((issuer_id, paid_tax)));
    }
}