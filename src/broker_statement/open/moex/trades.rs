use std::collections::HashMap;

use log::warn;
use serde::Deserialize;

use crate::broker_statement::cash_flows::{CashFlow, CashFlowType};
use crate::broker_statement::open::common::{deserialize_date, parse_quantity};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::broker_statement::trades::{StockBuy, StockSell};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::types::{Date, DateTime};
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::common::{deserialize_date_time, get_symbol};

#[derive(Deserialize)]
pub struct ConcludedTrades<const REPO: bool> {
    #[serde(rename = "item")]
    trades: Vec<ConcludedTrade<REPO>>,
}

impl<const REPO: bool> ConcludedTrades<REPO> {
    pub fn parse(
        &self, statement: &mut PartialBrokerStatement, securities: &HashMap<String, String>,
        trades_with_shifted_execution_date: &mut HashMap<u64, Date>,
    ) -> EmptyResult {
        for trade in &self.trades {
            let symbol = get_symbol(securities, &trade.security_name)?;
            trade.parse(statement, symbol, trades_with_shifted_execution_date)?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct ConcludedTrade<const REPO: bool> {
    #[serde(rename = "deal_no")]
    id: u64,

    security_name: String,

    #[serde(deserialize_with = "deserialize_date_time")]
    conclusion_time: DateTime,

    #[serde(deserialize_with = "deserialize_date")]
    execution_date: Date,

    #[serde(rename = "buy_qnty")]
    buy_quantity: Option<Decimal>,

    #[serde(rename = "sell_qnty")]
    sell_quantity: Option<Decimal>,

    price: Decimal,

    #[serde(rename = "price_currency_code")]
    price_currency: String,

    // volume - for repo trades
    // volume_currency - for ordinary trades
    #[serde(alias="volume_currency")]
    volume: Decimal,

    #[serde(rename = "accounting_currency_code")]
    accounting_currency: String,

    #[serde(rename = "broker_commission")]
    commission: Decimal,

    #[serde(rename = "broker_commission_currency_code")]
    commission_currency: Option<String>,
}

impl<const REPO: bool> ConcludedTrade<REPO> {
    fn parse(
        &self, statement: &mut PartialBrokerStatement, symbol: &str,
        trades_with_shifted_execution_date: &mut HashMap<u64, Date>,
    ) -> EmptyResult {
        // Just don't know which one exactly is
        if self.price_currency != self.accounting_currency {
            return Err!(
                "Trade currency for {} is not equal to accounting currency which is not supported yet",
                 symbol);
        }

        let price = util::validate_named_cash(
            "price", &self.price_currency, self.price,
            DecimalRestrictions::StrictlyPositive)?.normalize();

        let volume = util::validate_named_cash(
            "trade volume", &self.price_currency, self.volume,
            DecimalRestrictions::StrictlyPositive)?.normalize();

        let commission = util::validate_named_decimal(
            "commission", self.commission, DecimalRestrictions::PositiveOrZero)?;

        let commission_currency = match self.commission_currency.as_ref() {
            Some(currency) => currency,
            None if commission.is_zero() => &self.price_currency,
            None => return Err!("Missing commission currency for #{} trade", self.id),
        };
        let commission = Cash::new(commission_currency, commission);

        let execution_date = match trades_with_shifted_execution_date.remove(&self.id) {
            Some(execution_date) => {
                warn!(concat!(
                    "Actual execution date of #{} trade differs from the planned one. ",
                    "Fix execution date for this trade."
                ), self.id);
                execution_date
            },
            None => self.execution_date,
        };

        match (self.buy_quantity, self.sell_quantity) {
            (Some(quantity), None) => {
                let quantity = util::validate_decimal(
                    parse_quantity(quantity), DecimalRestrictions::StrictlyPositive)?;
                debug_assert_eq!(volume, price * quantity);

                if REPO {
                    statement.cash_flows.push(CashFlow::new(self.conclusion_time.into(), -volume, CashFlowType::Repo {
                        symbol: symbol.to_owned(),
                        commission,
                    }));
                } else {
                    statement.stock_buys.push(StockBuy::new_trade(
                        symbol, quantity, price, volume, commission,
                        self.conclusion_time.into(), execution_date));
                }
            },

            (None, Some(quantity)) => {
                let quantity = util::validate_decimal(
                    parse_quantity(quantity), DecimalRestrictions::StrictlyPositive)?;
                debug_assert_eq!(volume, price * quantity);

                if REPO {
                    statement.cash_flows.push(CashFlow::new(self.conclusion_time.into(), volume, CashFlowType::Repo {
                        symbol: symbol.to_owned(),
                        commission,
                    }));
                } else {
                    statement.stock_sells.push(StockSell::new_trade(
                        symbol, quantity, price, volume, commission,
                        self.conclusion_time.into(), execution_date, false));
                }
            },

            _ => return Err!("Got an unexpected trade: Can't match it as buy or sell trade"),
        }

        Ok(())
    }
}

#[derive(Deserialize)]
pub struct ExecutedTrades {
    #[serde(rename = "item")]
    trades: Vec<ExecutedTrade>,
}

#[derive(Deserialize)]
struct ExecutedTrade {
    #[serde(rename = "deal_no")]
    id: u64,

    #[serde(deserialize_with = "deserialize_date")]
    plan_execution_date: Date,

    #[serde(deserialize_with = "deserialize_date")]
    fact_execution_date: Date,
}

impl ExecutedTrades {
    pub fn parse(&self) -> GenericResult<HashMap<u64, Date>> {
        let mut trades_with_shifted_execution_date = HashMap::new();

        for trade in &self.trades {
            if trade.fact_execution_date != trade.plan_execution_date {
                if trades_with_shifted_execution_date.insert(trade.id, trade.fact_execution_date).is_some() {
                    return Err!("Got a duplicated {:?} trade", trade.id);
                }
            }
        }

        Ok(trades_with_shifted_execution_date)
    }
}

