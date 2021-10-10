use std::cmp::Ordering;
use std::collections::HashMap;

use lazy_static::lazy_static;
use num_traits::ToPrimitive;
use regex::Regex;
use serde::Deserialize;

use crate::broker_statement::corporate_actions::{CorporateAction, CorporateActionType, StockSplitRatio};
use crate::broker_statement::open::common::{deserialize_date, parse_quantity};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::{EmptyResult, GenericResult};
use crate::time::{Date, parse_date};
use crate::types::Decimal;
use crate::util::{DecimalRestrictions, validate_named_decimal};

use super::common::get_symbol;

#[derive(Deserialize)]
pub struct CorporateActions {
    #[serde(rename = "item")]
    operations: Vec<Operation>,
}

#[derive(Deserialize)]
struct Operation {
    #[serde(rename = "operation_date", deserialize_with = "deserialize_date")]
    date: Date,
    security_name: String,
    quantity: Decimal,
    comment: String,
}

impl CorporateActions {
    pub fn parse(&self, statement: &mut PartialBrokerStatement, securities: &HashMap<String, String>) -> EmptyResult {
        let mut index = 0;

        while index < self.operations.len() {
            let operation = &self.operations[index];
            index += 1;

            let operation_type = OperationType::parse(&operation.comment)?;
            let symbol = get_symbol(securities, &operation.security_name)?;

            match operation_type {
                OperationType::StockSplitWithdrawal(date) => {
                    let next_operation = self.operations.get(index);
                    index += 1;

                    statement.corporate_actions.push(parse_stock_split(
                        date, symbol, operation, next_operation)?);
                },

                OperationType::StockSplitDeposit(_) => {
                    return Err!("Got an unexpected corporate action: {}", operation.comment);
                }
            };
        }

        Ok(())
    }
}

fn parse_stock_split(
    date: Date, symbol: &str, operation: &Operation, next_operation: Option<&Operation>,
) -> GenericResult<CorporateAction> {
    let withdrawal = -validate_named_decimal(
        "stock split withdrawal quantity", parse_quantity(operation.quantity),
        DecimalRestrictions::StrictlyNegative)?;

    let next_operation = match next_operation {
        Some(next_operation) => {
            let next_operation_type = OperationType::parse(&next_operation.comment)?;

            if next_operation.date != operation.date || next_operation.security_name != operation.security_name ||
                !matches!(next_operation_type, OperationType::StockSplitDeposit(next_date) if next_date == date) {
                return Err!("Got an unexpected corporate action: {}", operation.comment);
            }

            next_operation
        },
        None => return Err!("Got an unexpected corporate action: {}", operation.comment),
    };

    let deposit = validate_named_decimal(
        "stock split deposit quantity", parse_quantity(next_operation.quantity),
        DecimalRestrictions::StrictlyPositive)?;

    let ratio = parse_ratio(withdrawal, deposit).ok_or_else(|| format!(
        "Got an unsupported corporate action: {}", operation.comment))?;

    Ok(CorporateAction {
        time: date.into(),
        report_date: Some(operation.date),

        symbol: symbol.to_owned(),
        action: CorporateActionType::StockSplit {
            ratio,
            from_change: Some(withdrawal),
            to_change: Some(deposit),
        },
    })
}

fn parse_ratio(withdrawal: Decimal, deposit: Decimal) -> Option<StockSplitRatio> {
    match withdrawal.cmp(&deposit) {
        Ordering::Less => (deposit / withdrawal).to_u32().and_then(|ratio| {
            if withdrawal * Decimal::from(ratio) == deposit {
                Some(StockSplitRatio::new(1, ratio))
            } else {
                None
            }
        }),
        Ordering::Greater => (withdrawal / deposit).to_u32().and_then(|ratio| {
            if withdrawal == deposit * Decimal::from(ratio) {
                Some(StockSplitRatio::new(ratio, 1))
            } else {
                None
            }
        }),
        Ordering::Equal => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum OperationType {
    StockSplitWithdrawal(Date),
    StockSplitDeposit(Date),
}

impl OperationType {
    fn parse(description: &str) -> GenericResult<OperationType> {
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
                "Снятие ЦБ с учета" => OperationType::StockSplitWithdrawal(date),
                "Прием ЦБ на учет"  => OperationType::StockSplitDeposit(date),
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
             OperationType::StockSplitWithdrawal(date!(2021, 10, 7))),
        case("Отчет депозитария б/н от 07.10.2021. Прием ЦБ на учет. Дробление - FinEx MSCI USA UCITS ETF-ип",
             OperationType::StockSplitDeposit(date!(2021, 10, 7))),
    )]
    fn operation_type_parsing(description: &str, expected: OperationType) {
        assert_eq!(OperationType::parse(description).unwrap(), expected);
    }
}