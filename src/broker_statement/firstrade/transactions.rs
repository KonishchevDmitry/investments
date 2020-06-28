use serde::Deserialize;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::EmptyResult;
use crate::currency::CashAssets;
use crate::types::{Date, Decimal};
use crate::util::{DecimalRestrictions, validate_decimal};

use super::common::{Ignore, deserialize_date, deserialize_decimal};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transactions {
    #[serde(rename = "DTSTART", deserialize_with = "deserialize_date")]
    pub start_date: Date,
    #[serde(rename = "DTEND", deserialize_with = "deserialize_date")]
    pub end_date: Date,
    // FIXME(konishchev): Support
    #[serde(rename = "INVBANKTRAN")]
    cash_flows: Vec<CashFlow>,
    #[serde(rename = "BUYSTOCK")]
    stock_buys: Vec<Ignore>,
    #[serde(rename = "SELLSTOCK")]
    stock_sells: Vec<Ignore>,
    #[serde(rename = "INCOME")]
    income: Vec<Ignore>,
}

impl Transactions {
    pub fn parse(self, statement: &mut PartialBrokerStatement, currency: &str) -> EmptyResult {
        for cash_flow in self.cash_flows {
            cash_flow.parse(statement, currency)?;
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CashFlow {
    #[serde(rename = "STMTTRN")]
    transaction: CashFlowTransaction,
    #[serde(rename = "SUBACCTFUND")]
    sub_account: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CashFlowTransaction {
    #[serde(rename = "TRNTYPE")]
    _type: String,
    #[serde(rename = "DTPOSTED", deserialize_with = "deserialize_date")]
    date: Date,
    #[serde(rename = "TRNAMT", deserialize_with = "deserialize_decimal")]
    amount: Decimal,
    #[serde(rename = "FITID")]
    id: String,
    #[serde(rename = "NAME")]
    name: Ignore,
}

impl CashFlow {
    pub fn parse(self, statement: &mut PartialBrokerStatement, currency: &str) -> EmptyResult {
        let transaction = self.transaction;

        if self.sub_account != "CASH" {
            return Err!(
                "Got an unsupported sub account type for {:?} cash flow transaction: {}",
                transaction.id, self.sub_account);
        }

        if transaction._type != "CREDIT" {
            return Err!(
                "Got an unsupported type of {:?} cash flow transaction: {}",
                transaction.id, transaction._type);
        }

        let amount = validate_decimal(transaction.amount, DecimalRestrictions::StrictlyPositive)?;
        statement.cash_flows.push(CashAssets::new(transaction.date, currency, amount));

        Ok(())
    }
}