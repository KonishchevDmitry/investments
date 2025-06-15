use serde::Deserialize;

use crate::broker_statement::open::common::{InstrumentType, deserialize_date, parse_quantity};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::broker_statement::trades::{StockBuy, StockSell};
use crate::core::EmptyResult;
use crate::exchanges::Exchange;
use crate::time::{Date, DateTime, Time};
use crate::types::{Decimal, TradeType};
use crate::util::{self, DecimalRestrictions};

use super::common::{deserialize_time, parse_security_code};

#[derive(Deserialize)]
pub struct Trades {
    #[serde(rename = "item")]
    trades: Vec<Trade>,
}

impl Trades {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for trade in &self.trades {
            trade.parse(statement)?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct Trade {
    #[serde(rename = "@ticketdate", deserialize_with = "deserialize_date")]
    conclusion_date: Date,
    #[serde(rename = "@tickettime", deserialize_with = "deserialize_time")]
    conclusion_time: Time,
    #[serde(rename = "@date3real", deserialize_with = "deserialize_date")]
    execution_date: Date,

    #[serde(rename = "@operationtype")]
    action: String,
    #[serde(rename = "@coderts")]
    security_code: String,
    #[serde(rename = "@categoryname")]
    category: String,
    #[serde(rename = "@place")]
    exchange: String,

    #[serde(rename = "@price")]
    price: Decimal,
    #[serde(rename = "@pricecurrency")]
    price_currency: String,
    #[serde(rename = "@quantity")]
    quantity: Decimal,
    #[serde(rename = "@paymentamount")]
    volume: Decimal,
    #[serde(rename = "@brokerage")]
    commission: Decimal,
    #[serde(rename = "@paymentcurrency")]
    payment_currency: String,
}

impl Trade {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        match InstrumentType::parse(&self.category)? {
            InstrumentType::Stock | InstrumentType::DepositaryReceipt => {},
        }

        let symbol = parse_security_code(&self.security_code)?;
        let conclusion_time = DateTime::new(self.conclusion_date, self.conclusion_time);

        let action = match self.action.as_str() {
            "Покупка" => TradeType::Buy,
            "Продажа" => TradeType::Sell,
            _ => return Err!("Unsupported trade operation: {:?}", self.action),
        };

        if self.price_currency != self.payment_currency {
            return Err!(
                "Trade currency for {} is not equal to payment currency which is not supported yet",
                symbol);
        }

        let price = util::validate_named_cash(
            "price", &self.price_currency, self.price,
            DecimalRestrictions::StrictlyPositive)?.normalize();

        let volume = util::validate_named_cash("trade volume", &self.payment_currency, match action {
            TradeType::Buy => -self.volume,
            TradeType::Sell => self.volume,
        }, DecimalRestrictions::StrictlyPositive)?.normalize();

        let quantity = util::validate_decimal(
            parse_quantity(self.quantity), DecimalRestrictions::StrictlyPositive)?;
        debug_assert_eq!(volume, price * quantity);

        let commission = util::validate_named_cash(
            "commission", &self.payment_currency, self.commission,
            DecimalRestrictions::PositiveOrZero)?.normalize();

        let exchange = match self.exchange.as_str() {
            "СПБ" => Exchange::Spb,
            _ => return Err!("Unknown exchange: {:?}", self.exchange),
        };
        statement.instrument_info.get_or_add(symbol).exchanges.add_prioritized(exchange);

        match action {
            TradeType::Buy => {
                statement.stock_buys.push(StockBuy::new_trade(
                    symbol, quantity, price, volume, commission,
                    conclusion_time.into(), self.execution_date));
            },
            TradeType::Sell => {
                statement.stock_sells.push(StockSell::new_trade(
                    symbol, quantity, price, volume, commission,
                    conclusion_time.into(), self.execution_date, false));
            },
        }

        Ok(())
    }
}