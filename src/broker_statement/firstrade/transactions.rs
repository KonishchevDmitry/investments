use num_traits::cast::ToPrimitive;
use serde::Deserialize;

use crate::broker_statement::{StockBuy, StockSell};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::EmptyResult;
use crate::currency::{Cash, CashAssets};
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

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
    #[serde(rename = "BUYSTOCK")]
    stock_buys: Vec<StockBuyInfo>,
    #[serde(rename = "SELLSTOCK")]
    stock_sells: Vec<StockSellInfo>,
    // FIXME(konishchev): Support
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
            if stock_buy._type != "BUY" {
                return Err!("Got an unsupported type of stock purchase: {:?}", stock_buy._type);
            }
            stock_buy.transaction.parse(statement, currency, securities, true)?;
        }

        for stock_sell in self.stock_sells {
            if stock_sell._type != "SELL" {
                return Err!("Got an unsupported type of stock sell: {:?}", stock_sell._type);
            }
            stock_sell.transaction.parse(statement, currency, securities, false)?;
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

        let amount = util::validate_named_decimal(
            "transaction amount", transaction.amount, DecimalRestrictions::StrictlyPositive)?;
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
    transaction: StockTradeTransaction,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StockSellInfo {
    #[serde(rename = "SELLTYPE")]
    _type: String,
    #[serde(rename = "INVSELL")]
    transaction: StockTradeTransaction,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StockTradeTransaction {
    #[serde(rename = "INVTRAN")]
    info: StockTransactionInfo,
    #[serde(rename = "SECID")]
    security_id: SecurityId,
    #[serde(rename = "UNITS")]
    units: String,
    #[serde(rename = "UNITPRICE", deserialize_with = "deserialize_decimal")]
    price: Decimal,
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
struct StockTransactionInfo {
    #[serde(rename = "FITID")]
    id: Ignore,
    #[serde(rename = "DTTRADE", deserialize_with = "deserialize_date")]
    conclusion_date: Date,
    #[serde(rename = "DTSETTLE", deserialize_with = "deserialize_date")]
    execution_date: Date,
    #[serde(rename = "MEMO")]
    memo: Ignore,
}

impl StockTradeTransaction {
    pub fn parse(
        self, statement: &mut PartialBrokerStatement, currency: &str, securities: &SecurityInfo,
        buy: bool,
    ) -> EmptyResult {
        validate_sub_account(&self.sub_account_from)?;
        validate_sub_account(&self.sub_account_to)?;

        let symbol = match securities.get(&self.security_id)? {
            SecurityType::Stock(symbol) => symbol,
            _ => return Err!("Got {} stock trade with an unexpected security type", self.security_id),
        };

        let quantity = util::parse_decimal(
            &self.units, if buy {
                DecimalRestrictions::StrictlyPositive
            } else {
                DecimalRestrictions::StrictlyNegative
            })
            .ok().and_then(|quantity| {
                if quantity.trunc() == quantity {
                    quantity.abs().to_u32()
                } else {
                    None
                }
            })
            .ok_or_else(|| format!("Invalid trade quantity: {:?}", self.units))?;

        let price = util::validate_named_decimal(
            "price", self.price, DecimalRestrictions::StrictlyPositive)
            .map(|price| Cash::new(currency, price.normalize()))?;

        let commission = util::validate_named_decimal(
            "commission", self.commission, DecimalRestrictions::PositiveOrZero
        ).and_then(|commission| {
            let fees = util::validate_named_decimal(
                "fees", self.fees, DecimalRestrictions::PositiveOrZero)?;
            Ok(commission + fees)
        }).map(|commission| Cash::new(currency, commission))?;

        let volume = util::validate_named_decimal(
            "trade volume", self.total, if buy {
                DecimalRestrictions::StrictlyNegative
            } else {
                DecimalRestrictions::StrictlyPositive
            })
            .map(|mut volume| {
                volume = volume.abs();

                if buy {
                    volume -= commission.amount;
                } else {
                    volume += commission.amount
                }

                Cash::new(currency, volume)
            })?;
        debug_assert_eq!(volume, (price * quantity).round());

        // FIXME(konishchev): Enable
        if true {
        } else if buy {
            statement.stock_buys.push(StockBuy::new(
                &symbol, quantity, price, volume, commission,
                self.info.conclusion_date, self.info.execution_date));
        } else {
            statement.stock_sells.push(StockSell::new(
                &symbol, quantity, price, volume, commission,
                self.info.conclusion_date, self.info.execution_date, false));
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