mod assets;
mod cash_flows;
mod common;
mod corporate_actions;
mod forex;
mod securities;
mod trades;

use std::collections::HashMap;

use log::error;
use serde::Deserialize;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::core::GenericResult;
use crate::exchanges::Exchange;
use crate::time::{Date, Period};

use super::common::deserialize_date;

use assets::{AccountSummary, Assets};
use cash_flows::CashFlows;
use corporate_actions::CorporateActions;
use forex::{CurrencyConversions, ForexTrades};
use securities::Securities;
use trades::{ConcludedTrades, ExecutedTrades};

#[derive(Deserialize)]
pub struct BrokerReport {
    #[serde(rename = "@date_from", deserialize_with = "deserialize_date")]
    date_from: Date,

    #[serde(rename = "@date_to", deserialize_with = "deserialize_date")]
    date_to: Date,

    #[serde(rename = "spot_account_totally", alias = "unified_account_totally")]
    account_summary: AccountSummary,

    #[serde(rename = "spot_assets", alias = "unified_closing_assets")]
    assets: Option<Assets>,

    #[serde(rename = "made_deal")]
    forex_trades: Option<ForexTrades>,

    #[serde(rename = "spot_nonstock_conversion_deals_conclusion")]
    currency_conversions: Option<CurrencyConversions>,

    #[serde(rename = "spot_main_deals_conclusion")]
    concluded_trades: Option<ConcludedTrades<false>>,

    #[serde(rename = "spot_repo_deals_conclusion")]
    concluded_repo_trades: Option<ConcludedTrades<true>>,

    #[serde(rename = "spot_main_deals_executed")]
    executed_trades: Option<ExecutedTrades>,

    #[serde(rename = "spot_non_trade_money_operations", alias = "unified_non_trade_money_operations")]
    cash_flow: Option<CashFlows>,

    #[serde(rename = "spot_non_trade_security_operations")]
    corporate_actions: Option<CorporateActions>,

    #[serde(rename = "spot_portfolio_security_params")]
    securities: Option<Securities>,
}

impl BrokerReport {
    pub fn parse(&self) -> GenericResult<PartialBrokerStatement> {
        let mut statement = PartialBrokerStatement::new(&[Exchange::Moex], true);
        statement.period.replace(Period::new(self.date_from, self.date_to)?);

        let securities = if let Some(ref securities) = self.securities {
            securities.parse(&mut statement)?
        } else {
            HashMap::new()
        };

        let mut has_starting_assets = self.account_summary.parse()?;
        if let Some(ref assets) = self.assets {
            has_starting_assets |= assets.parse(&mut statement, &securities)?;
        }
        statement.set_has_starting_assets(has_starting_assets)?;

        if let Some(ref trades) = self.forex_trades {
            trades.parse(&mut statement)?;
        }

        if let Some(ref conversion) = self.currency_conversions {
            conversion.parse(&mut statement)?;
        }

        let mut trades_with_shifted_execution_date = if let Some(ref trades) = self.executed_trades {
            trades.parse()?
        } else {
            HashMap::new()
        };

        if let Some(ref trades) = self.concluded_trades {
            trades.parse(&mut statement, &securities, &mut trades_with_shifted_execution_date)?;
        }

        if let Some(ref trades) = self.concluded_repo_trades {
            trades.parse(&mut statement, &securities, &mut HashMap::new())?;
        }

        if let Some(ref cash_flow) = self.cash_flow {
            cash_flow.parse(&mut statement)?;
        }

        if let Some(ref corporate_actions) = self.corporate_actions {
            corporate_actions.parse(&mut statement, &securities)?;
        }

        // Actually, we should check trade execution dates on statements merging stage when we have
        // a full view, but it would add an extra unneeded complexity to its generic logic. So now
        // we just do our best here and log found cases. If they actually will - we'll generalize
        // statements merging logic and add an ability to consider such things in the right place.
        if !trades_with_shifted_execution_date.is_empty() {
            let trade_ids = trades_with_shifted_execution_date.keys()
                .map(|trade_id| trade_id.to_string())
                .collect::<Vec<_>>();

            error!(concat!(
                "Actual execution date of the following trades differs from the planned one and ",
                "can't be fixed: {}."), trade_ids.join(", "));
        }

        Ok(statement)
    }
}