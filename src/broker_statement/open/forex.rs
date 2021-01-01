use serde::Deserialize;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::broker_statement::trades::ForexTrade;
use crate::core::EmptyResult;
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

use super::common::deserialize_date;

#[derive(Deserialize)]
pub struct ForexTrades {
    #[serde(rename = "item")]
    trades: Vec<ForexTradeInfo>,
}

// Please note:
// It's actually T+1 currency conversion trade. But for now consider it as T+0 forex trade.
#[derive(Deserialize)]
struct ForexTradeInfo {
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

impl ForexTrades {
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