mod plans;

use std::collections::BTreeMap;

use matches::matches;
use serde::Deserialize;
use serde::de::{Deserializer, Error as _};

use crate::broker_statement::StatementsMergingStrategy;
use crate::commissions::CommissionSpec;
use crate::config::{Config, BrokersConfig, BrokerConfig};
use crate::core::GenericResult;
use crate::currency::CashAssets;
use crate::types::Decimal;

#[derive(Debug, Clone, Copy)]
pub enum Broker {
    Bcs,
    Firstrade,
    InteractiveBrokers,
    Open,
    Tinkoff,
}

impl Broker {
    pub fn get_info(self, config: &Config, plan: Option<&String>) -> GenericResult<BrokerInfo> {
        let config = config.brokers.as_ref()
            .and_then(|brokers| self.get_config(brokers))
            .ok_or_else(|| format!(
                "{} configuration is not set in the configuration file", self.get_name()))?
            .clone();

        let statements_merging_strategy = match self {
            Broker::Bcs => StatementsMergingStrategy::Sparse,
            Broker::InteractiveBrokers => StatementsMergingStrategy::SparseOnHolidays(1),
            _ => StatementsMergingStrategy::ContinuousOnly,
        };

        Ok(BrokerInfo {
            type_: self,
            name: self.get_name(),
            config: config,
            commission_spec: self.get_commission_spec(plan)?,
            allow_future_fees: matches!(self, Broker::Tinkoff),
            statements_merging_strategy: statements_merging_strategy,
        })
    }

    fn get_name(self) -> &'static str {
        match self {
            Broker::Bcs => "ООО «Компания БКС»",
            Broker::Firstrade => "Firstrade Securities Inc.",
            Broker::InteractiveBrokers => "Interactive Brokers LLC",
            Broker::Open => "АО «Открытие Брокер»",
            Broker::Tinkoff => "АО «Тинькофф Банк»",
        }
    }

    fn get_config(self, config: &BrokersConfig) -> Option<&BrokerConfig> {
        match self {
            Broker::Bcs => &config.bcs,
            Broker::Firstrade => &config.firstrade,
            Broker::InteractiveBrokers => &config.interactive_brokers,
            Broker::Open => &config.open_broker,
            Broker::Tinkoff => &config.tinkoff,
        }.as_ref()
    }

    fn get_commission_spec(self, plan: Option<&String>) -> GenericResult<CommissionSpec> {
        type PlanFn = fn() -> CommissionSpec;

        let (default, plans): (PlanFn, BTreeMap<&str, PlanFn>) = match self {
            Broker::Bcs => (plans::bcs::professional, btreemap!{
                "Профессиональный" => plans::bcs::professional as PlanFn,
            }),
            Broker::Firstrade => (plans::firstrade::free, btreemap!{}),
            Broker::InteractiveBrokers => (plans::ib::fixed, btreemap!{
                "Fixed" => plans::ib::fixed as PlanFn,
            }),
            Broker::Open => (plans::open::iia, btreemap!{
                "Самостоятельное управление (ИИС)" => plans::open::iia as PlanFn,
            }),
            Broker::Tinkoff => (plans::tinkoff::trader, btreemap!{
                "Трейдер" => plans::tinkoff::trader as PlanFn,
            }),
        };

        let plan = match plan {
            Some(plan) => {
                *plans.get(plan.as_str()).ok_or_else(|| format!(
                    "Invalid plan for {}: {}. Available plans: {}",
                    self.get_name(), plan, plans.keys().copied().collect::<Vec<_>>().join(", "),
                ))?
            },
            None => default,
        };

        Ok(plan())
    }
}

impl<'de> Deserialize<'de> for Broker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;

        Ok(match value.as_str() {
            "bcs" => Broker::Bcs,
            "firstrade" => Broker::Firstrade,
            "interactive-brokers" => Broker::InteractiveBrokers,
            "open-broker" => Broker::Open,
            "tinkoff" => Broker::Tinkoff,

            _ => return Err(D::Error::unknown_variant(&value, &[
                "bcs", "firstrade", "interactive-brokers", "open-broker", "tinkoff",
            ])),
        })
    }
}

#[derive(Debug)]
pub struct BrokerInfo {
    pub type_: Broker,
    pub name: &'static str,

    config: BrokerConfig,
    pub commission_spec: CommissionSpec,
    pub allow_future_fees: bool,
    pub statements_merging_strategy: StatementsMergingStrategy,
}

impl BrokerInfo {
    pub fn get_deposit_commission(&self, assets: CashAssets) -> GenericResult<Decimal> {
        let currency = assets.cash.currency;

        let commission_spec = match self.config.deposit_commissions.get(currency) {
            Some(commission_spec) => commission_spec,
            None => return Err!(concat!(
                "Unable to calculate commission for {} deposit to {}: there is no commission ",
                "specification in the configuration file"), currency, self.name),
        };

        Ok(commission_spec.fixed_amount)
    }
}