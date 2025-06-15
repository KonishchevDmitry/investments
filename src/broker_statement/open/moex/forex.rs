use serde::Deserialize;

use crate::broker_statement::open::common::deserialize_date;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::broker_statement::trades::ForexTrade;
use crate::core::EmptyResult;
use crate::forex::parse_forex_code;
use crate::time::{Date, Time, DateTime};
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::common::{deserialize_time, deserialize_date_time};

#[derive(Deserialize)]
pub struct ForexTrades {
    #[serde(rename = "item")]
    trades: Vec<ForexTradeInfo>,
}

#[derive(Deserialize)]
struct ForexTradeInfo {
    #[serde(rename = "@deal_date", deserialize_with = "deserialize_date")]
    conclusion_date: Date,

    #[serde(rename = "@deal_time", deserialize_with = "deserialize_time")]
    conclusion_time: Time,

    #[serde(rename = "@contract_code")]
    code: String,

    #[serde(rename = "@exec_symbol")]
    action: String,

    #[serde(rename = "@quantity")]
    quantity: Decimal,

    #[serde(rename = "@price_plan")]
    price: Decimal,

    #[serde(rename = "@currency_code")]
    currency: String,

    #[serde(rename = "@volume")]
    volume: Decimal,

    #[serde(rename = "@broker_comm")]
    commission: Decimal,

    #[serde(rename = "@broker_comm_currency_code")]
    commission_currency: String,
}

impl ForexTrades {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for trade in &self.trades {
            let time = DateTime::new(trade.conclusion_date, trade.conclusion_time);

            let (base, quote, lot_size) = parse_forex_code(&trade.code)?;
            if quote != trade.currency {
                return Err!(
                    "Got an unexpected forex quote currency for {}: {}",
                    trade.code, trade.currency);
            }

            let lot_size = lot_size.ok_or_else(|| format!(
                "{} currency pair is not supported yet", trade.code))?;

            let quantity = util::validate_named_cash(
                "forex trade volume", base, trade.quantity * lot_size,
                DecimalRestrictions::StrictlyPositive)?.normalize();

            let price = util::validate_named_cash(
                "forex trade price", quote, trade.price,
                DecimalRestrictions::StrictlyPositive)?.normalize();

            let volume = util::validate_named_cash(
                "forex trade volume", quote, trade.volume,
                DecimalRestrictions::StrictlyPositive)?.normalize();

            let expected_volume = price * quantity.amount;
            if volume.round() != expected_volume.round() {
                return Err!(
                    "Got an unexpected forex trade volume: {} vs {}",
                    volume, expected_volume);
            }
            debug_assert_eq!(volume, price * quantity.amount);

            let (from, to) = match trade.action.as_str() {
                "B" => (volume, quantity),
                "S" => (quantity, volume),
                _ => return Err!("Unsupported forex trade action type: {:?}", trade.action),
            };

            let commission = util::validate_named_cash(
                "commission", parse_currency(&trade.commission_currency), trade.commission,
                DecimalRestrictions::PositiveOrZero)?.normalize();

            statement.forex_trades.push(ForexTrade::new(time.into(), from, to, commission));
        }

        Ok(())
    }
}

#[derive(Deserialize)]
pub struct CurrencyConversions {
    #[serde(rename = "item")]
    trades: Vec<CurrencyConversionInfo>,
}

// Please note:
// It's actually T+1 currency conversion trade. But for now consider it as T+0 forex trade.
#[derive(Deserialize)]
struct CurrencyConversionInfo {
    #[serde(rename = "@conclusion_time", deserialize_with = "deserialize_date_time")]
    conclusion_time: DateTime,

    #[serde(rename = "@direction")]
    direction: String,

    #[serde(rename = "@currency_1")]
    currency_1: String,

    #[serde(rename = "@sum_1")]
    volume_1: Decimal,

    #[serde(rename = "@currency_2")]
    currency_2: String,

    #[serde(rename = "@sum_2")]
    volume_2: Decimal,

    #[serde(rename = "@brokers_fee")]
    commission: Decimal,

    #[serde(rename = "@brokers_fee_currency")]
    commission_currency: String,
}

impl CurrencyConversions {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for trade in &self.trades {
            let volume_1 = util::validate_named_cash(
                "forex trade volume", parse_currency(&trade.currency_1), trade.volume_1,
                DecimalRestrictions::StrictlyPositive)?.normalize();

            let volume_2 = util::validate_named_cash(
                "forex trade volume", parse_currency(&trade.currency_2), trade.volume_2,
                DecimalRestrictions::StrictlyPositive)?.normalize();

            let commission = util::validate_named_cash(
                "commission", parse_currency(&trade.commission_currency), trade.commission,
                DecimalRestrictions::PositiveOrZero)?.normalize();

            let (from, to) = match trade.direction.as_str() {
                "B" => (volume_2, volume_1),
                "S" => (volume_1, volume_2),
                _ => return Err!("Unsupported currency conversion direction: {:?}", trade.direction),
            };

            statement.forex_trades.push(ForexTrade::new(
                trade.conclusion_time.into(), from, to, commission));
        }

        Ok(())
    }
}

fn parse_currency(currency: &str) -> &str {
    if currency == "RUR" {
        "RUB"
    } else {
        currency
    }
}