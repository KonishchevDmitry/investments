use serde::Deserialize;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::broker_statement::trades::ForexTrade;
use crate::core::EmptyResult;
use crate::forex::parse_forex_code;
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

use super::common::deserialize_date;

#[derive(Deserialize)]
pub struct ForexTrades {
    #[serde(rename = "item")]
    trades: Vec<ForexTradeInfo>,
}

#[derive(Deserialize)]
struct ForexTradeInfo {
    #[serde(rename = "deal_date", deserialize_with = "deserialize_date")]
    conclusion_date: Date,

    #[serde(rename = "contract_code")]
    code: String,

    #[serde(rename = "exec_symbol")]
    action: String,

    #[serde(rename = "quantity")]
    quantity: Decimal,

    #[serde(rename = "price_plan")]
    price: Decimal,

    #[serde(rename = "currency_code")]
    currency: String,

    #[serde(rename = "volume")]
    volume: Decimal,

    #[serde(rename = "broker_comm")]
    commission: Decimal,

    #[serde(rename = "broker_comm_currency_code")]
    commission_currency: String,
}

impl ForexTrades {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for trade in &self.trades {
            let (base, quote, lot_size) = parse_forex_code(&trade.code)?;
            if quote != trade.currency {
                return Err!(
                    "Got an unexpected forex quote currency for {}: {}",
                    trade.code, trade.currency);
            }

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

            statement.forex_trades.push(ForexTrade {
                from, to, commission,
                conclusion_date: trade.conclusion_date
            })
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
    #[serde(deserialize_with = "deserialize_date")]
    conclusion_date: Date,

    #[serde(rename = "currency_1")]
    from_currency: String,

    #[serde(rename = "sum_1")]
    from_volume: Decimal,

    #[serde(rename = "currency_2")]
    to_currency: String,

    #[serde(rename = "sum_2")]
    to_volume: Decimal,

    #[serde(rename = "brokers_fee")]
    commission: Decimal,

    #[serde(rename = "brokers_fee_currency")]
    commission_currency: String,
}

impl CurrencyConversions {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for trade in &self.trades {
            let from = util::validate_named_cash(
                "forex trade volume", parse_currency(&trade.from_currency), trade.from_volume,
                DecimalRestrictions::StrictlyPositive)?.normalize();

            let to = util::validate_named_cash(
                "forex trade volume", parse_currency(&trade.to_currency), trade.to_volume,
                DecimalRestrictions::StrictlyPositive)?.normalize();

            let commission = util::validate_named_cash(
                "commission", parse_currency(&trade.commission_currency), trade.commission,
                DecimalRestrictions::PositiveOrZero)?.normalize();

            statement.forex_trades.push(ForexTrade {
                from, to, commission,
                conclusion_date: trade.conclusion_date
            })
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