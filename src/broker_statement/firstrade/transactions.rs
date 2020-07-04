use num_traits::cast::ToPrimitive;
use serde::Deserialize;

use crate::broker_statement::StockBuy;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::EmptyResult;
use crate::currency::{Cash, CashAssets};
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions, validate_decimal};

use super::common::{Ignore, deserialize_date, deserialize_decimal};
use super::security_info::{SecurityInfo, SecurityId, SecurityType};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transactions {
    #[serde(rename = "DTSTART", deserialize_with = "deserialize_date")]
    pub start_date: Date,
    #[serde(rename = "DTEND", deserialize_with = "deserialize_date")]
    pub end_date: Date,
    #[serde(rename = "INVBANKTRAN")]
    cash_flows: Vec<CashFlowInfo>,
    // FIXME(konishchev): Support
    #[serde(rename = "BUYSTOCK")]
    stock_buys: Vec<StockBuyInfo>,
    #[serde(rename = "SELLSTOCK")]
    stock_sells: Vec<Ignore>,
    #[serde(rename = "INCOME")]
    income: Vec<Ignore>,
}

impl Transactions {
    pub fn parse(
        self, statement: &mut PartialBrokerStatement, currency: &str, securities: &SecurityInfo,
    ) -> EmptyResult {
        for cash_flow in self.cash_flows {
            cash_flow.parse(statement, currency)?;
        }

        for stock_buy in self.stock_buys {
            stock_buy.parse(statement, currency, securities)?;
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CashFlowInfo {
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
    id: Ignore,
    #[serde(rename = "NAME")]
    name: Ignore,
}

impl CashFlowInfo {
    pub fn parse(self, statement: &mut PartialBrokerStatement, currency: &str) -> EmptyResult {
        let transaction = self.transaction;

        if transaction._type != "CREDIT" {
            return Err!(
                "Got an unsupported type of {:?} cash flow transaction: {}",
                transaction.id, transaction._type);
        }
        validate_sub_account(&self.sub_account)?;

        let amount = validate_decimal(transaction.amount, DecimalRestrictions::StrictlyPositive)?;
        statement.cash_flows.push(CashAssets::new(transaction.date, currency, amount));

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StockBuyInfo {
    #[serde(rename = "BUYTYPE")]
    _type: String,
    #[serde(rename = "INVBUY")]
    transaction: StockBuyTransaction,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StockBuyTransaction {
    #[serde(rename = "INVTRAN")]
    info: TransactionInfo,
    #[serde(rename = "SECID")]
    security_id: SecurityId,
    #[serde(rename = "UNITS", deserialize_with = "deserialize_decimal")]
    units: Decimal,
    #[serde(rename = "UNITPRICE", deserialize_with = "deserialize_decimal")]
    price: Decimal,
    // FIXME(konishchev): Support
    #[serde(rename = "COMMISSION", deserialize_with = "deserialize_decimal")]
    commission: Decimal,
    #[serde(rename = "FEES", deserialize_with = "deserialize_decimal")]
    fees: Decimal,
    #[serde(rename = "TOTAL", deserialize_with = "deserialize_decimal")]
    total: Decimal,
    #[serde(rename = "SUBACCTSEC")]
    sub_account_to: String,
    #[serde(rename = "SUBACCTFUND")]
    sub_account_from: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransactionInfo {
    #[serde(rename = "FITID")]
    id: Ignore,
    #[serde(rename = "DTTRADE", deserialize_with = "deserialize_date")]
    conclusion_date: Date,
    #[serde(rename = "DTSETTLE", deserialize_with = "deserialize_date")]
    execution_date: Date,
    #[serde(rename = "MEMO")]
    memo: Ignore,
}

impl StockBuyInfo {
    pub fn parse(
        self, statement: &mut PartialBrokerStatement, currency: &str, securities: &SecurityInfo,
    ) -> EmptyResult {
        if self._type != "BUY" {
            return Err!("Got an unsupported type of stock purchase: {:?}", self._type);
        }

        let transaction = self.transaction;

        let symbol = match securities.get(&transaction.security_id)? {
            SecurityType::Stock(symbol) => symbol,
            _ => return Err!(
                "Got {} stock buy with an unexpected security type", transaction.security_id),
        };

        let quantity = validate_decimal(transaction.units, DecimalRestrictions::StrictlyPositive)
            .ok().and_then(|quantity| {
                if quantity.trunc() == quantity {
                    quantity.to_u32()
                } else {
                    None
                }
            })
            .ok_or_else(|| format!("Got an invalid {} buy quantity: {:?}", symbol, transaction.units))?;
        let price = validate_decimal(transaction.price, DecimalRestrictions::StrictlyPositive)?;
        let commission =
            validate_decimal(transaction.commission, DecimalRestrictions::PositiveOrZero)? +
            validate_decimal(transaction.fees, DecimalRestrictions::PositiveOrZero)?;

        let volume = -validate_decimal(transaction.total, DecimalRestrictions::StrictlyNegative)?;
        debug_assert_eq!(volume, util::round(price * Decimal::from(quantity), 2));

        // FIXME(konishchev): Enable
        if false {
            statement.stock_buys.push(StockBuy::new(
                &symbol, quantity,
                Cash::new(currency, price), Cash::new(currency, volume), Cash::new(currency, commission),
                transaction.info.conclusion_date, transaction.info.execution_date));
        }

        Ok(())
    }
}

fn validate_sub_account(name: &str) -> EmptyResult {
    match name {
        "CASH" => Ok(()),
        _ => Err!("Got an unsupported sub-account type: {:?}", name),
    }
}