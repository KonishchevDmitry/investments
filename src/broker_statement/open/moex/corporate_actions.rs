// FIXME(konishchev): Remove
#![allow(dead_code)]

use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;

use crate::broker_statement::open::common::deserialize_date;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::{EmptyResult, GenericResult};
use crate::time::{Date, parse_date};
use crate::types::Decimal;

#[derive(Deserialize)]
pub struct CorporateActions {
    #[serde(rename = "item")]
    actions: Vec<CorporateActionItem>,
}

impl CorporateActions {
    // FIXME(konishchev): Implement
    pub fn parse(&self, _statement: &mut PartialBrokerStatement) -> EmptyResult {
        Ok(())
    }
}

#[derive(Deserialize)]
struct CorporateActionItem {
    #[serde(rename = "operation_date", deserialize_with = "deserialize_date")]
    date: Date,
    security_name: String,
    quantity: Decimal,
    comment: String,
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    StockSplitWithdrawal(Date),
    StockSplitDeposit(Date),
}

impl Action {
    fn parse(description: &str) -> GenericResult<Action> {
        lazy_static! {
            static ref STOCK_SPLIT_REGEX: Regex = Regex::new(concat!(
                r"^Отчет депозитария б/н от (?P<date>\d{2}.\d{2}.\d{4}). ",
                r"(?P<action>Снятие ЦБ с учета|Прием ЦБ на учет). ",
                r"Дробление - "
            )).unwrap();
        }

        if let Some(captures) = STOCK_SPLIT_REGEX.captures(description) {
            let date = parse_date(captures.name("date").unwrap().as_str(), "%d.%m.%Y")?;
            let action = captures.name("action").unwrap().as_str();
            return Ok(match action {
                "Снятие ЦБ с учета" => Action::StockSplitWithdrawal(date),
                "Прием ЦБ на учет"  => Action::StockSplitDeposit(date),
                _ => unreachable!(),
            })
        }

        return Err!("Unsupported corporate action: {:?}", description);
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(description, expected,
        case("Отчет депозитария б/н от 07.10.2021. Снятие ЦБ с учета. Дробление - FinEx MSCI USA UCITS ETF-ип",
             Action::StockSplitWithdrawal(date!(2021, 10, 7))),
        case("Отчет депозитария б/н от 07.10.2021. Прием ЦБ на учет. Дробление - FinEx MSCI USA UCITS ETF-ип",
             Action::StockSplitDeposit(date!(2021, 10, 7))),
    )]
    fn corporate_action_parsing(description: &str, expected: Action) {
        assert_eq!(Action::parse(description).unwrap(), expected);
    }
}