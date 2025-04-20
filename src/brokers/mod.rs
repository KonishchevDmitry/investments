pub mod config;
pub mod plans;

use std::collections::BTreeMap;

use matches::matches;
use serde::Deserialize;
use serde::de::{Deserializer, Error as _};

use crate::broker_statement::StatementsMergingStrategy;
use crate::commissions::CommissionSpec;
use crate::config::Config;
use crate::core::GenericResult;
use crate::currency::{Cash, CashAssets};
use crate::exchanges::Exchange;
use crate::localities::{Country, Jurisdiction};

use self::config::{BrokersConfig, BrokerConfig};

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy)]
pub enum Broker {
    Bcs,
    Firstrade,
    InteractiveBrokers,
    Open,
    Sber,
    Tbank,
}

impl Broker {
    pub fn id(self) -> &'static str {
        match self {
            Broker::Bcs => "bcs",
            Broker::Firstrade => "firstrade",
            Broker::InteractiveBrokers => "interactive-brokers",
            Broker::Open => "open",
            Broker::Sber => "sber",
            Broker::Tbank => "tbank",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Broker::Bcs => "ООО «Компания БКС»",
            Broker::Firstrade => "Firstrade Securities Inc.",
            Broker::InteractiveBrokers => "Interactive Brokers LLC",
            Broker::Open => "АО «Открытие Брокер»",
            Broker::Sber => "ПАО «Сбербанк»",
            Broker::Tbank => "АО «ТБанк»",
        }
    }

    pub fn brief_name(self) -> &'static str {
        match self {
            Broker::Bcs => "БКС",
            Broker::Firstrade => "Firstrade",
            Broker::InteractiveBrokers => "Interactive Brokers",
            Broker::Open => "Открытие",
            Broker::Sber => "Сбер",
            Broker::Tbank => "Т‑Банк",
        }
    }

    pub fn jurisdiction(self) -> Jurisdiction {
        match self {
            Broker::Bcs | Broker::Open | Broker::Sber | Broker::Tbank => Jurisdiction::Russia,
            Broker::Firstrade | Broker::InteractiveBrokers => Jurisdiction::Usa,
        }
    }

    pub fn get_info(self, config: &Config, plan: Option<&str>) -> GenericResult<BrokerInfo> {
        let config = config.brokers.as_ref()
            .and_then(|brokers| self.get_config(brokers).cloned())
            .unwrap_or_default();

        let statements_merging_strategy = match self {
            Broker::InteractiveBrokers => StatementsMergingStrategy::SparseOnHolidays(1),
            Broker::Open => StatementsMergingStrategy::SparseSingleDaysLastMonth(0),
            Broker::Sber => StatementsMergingStrategy::Sparse,
            _ => StatementsMergingStrategy::ContinuousOnly,
        };

        Ok(BrokerInfo {
            type_: self,
            name: self.name(),
            brief_name: self.brief_name(),

            config: config,
            commission_spec: self.get_commission_spec(plan)?,
            allow_future_fees: matches!(self, Broker::Tbank),
            fractional_shares_trading: matches!(self, Broker::InteractiveBrokers),
            statements_merging_strategy: statements_merging_strategy,
        })
    }

    pub fn get_commission_spec(self, plan: Option<&str>) -> GenericResult<CommissionSpec> {
        type PlanFn = fn() -> CommissionSpec;

        let (default, plans): (PlanFn, BTreeMap<&str, PlanFn>) = match self {
            Broker::Bcs => (plans::bcs::investor, btreemap!{
                "Инвестор" => plans::bcs::investor as PlanFn,
                "Трейдер" => plans::bcs::trader as PlanFn,

                "Инвестор Про" => plans::bcs::investor_pro_deprecated as PlanFn,
                "Профессиональный" => plans::bcs::professional_deprecated as PlanFn,
            }),

            Broker::Firstrade => (plans::firstrade::free, btreemap!{}),

            Broker::InteractiveBrokers => (plans::ib::fixed, btreemap!{
                "Fixed" => plans::ib::fixed as PlanFn,
            }),

            Broker::Open => (plans::open::all_inclusive, btreemap!{
                "Всё включено" => plans::open::all_inclusive as PlanFn,
                "Самостоятельное управление (ИИС)" => plans::open::iia as PlanFn,
            }),

            Broker::Sber => (plans::sber::investment, btreemap!{
                "Инвестиционный" => plans::sber::investment as PlanFn,
                "Самостоятельный" => plans::sber::manual as PlanFn,
            }),

            Broker::Tbank => (plans::tbank::investor, btreemap!{
                "Инвестор" => plans::tbank::investor as PlanFn,
                "Трейдер" => plans::tbank::trader as PlanFn,
                "Премиум" => plans::tbank::premium as PlanFn,
            }),
        };

        let plan = match plan {
            Some(plan) => {
                *plans.get(plan).ok_or_else(|| format!(
                    "Invalid plan for {}: {}. Available plans: {}",
                    self.name(), plan, plans.keys().copied().collect::<Vec<_>>().join(", "),
                ))?
            },
            None => default,
        };

        Ok(plan())
    }

    fn get_config(self, config: &BrokersConfig) -> Option<&BrokerConfig> {
        match self {
            Broker::Bcs => config.bcs.as_ref(),
            Broker::Firstrade => config.firstrade.as_ref(),
            Broker::InteractiveBrokers => config.interactive_brokers.as_ref(),
            Broker::Open => config.open_broker.as_ref(),
            Broker::Sber => config.sber.as_ref(),
            Broker::Tbank => config.tbank.as_ref().and_then(|tbank| tbank.broker.as_ref()),
        }
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
            "sber" => Broker::Sber,
            "tbank" => Broker::Tbank,
            "tinkoff" => Broker::Tbank,

            _ => return Err(D::Error::unknown_variant(&value, &[
                "bcs", "firstrade", "interactive-brokers", "open-broker", "sber", "tbank",
            ])),
        })
    }
}

#[derive(Clone)]
pub struct BrokerInfo {
    pub type_: Broker,
    pub name: &'static str,
    pub brief_name: &'static str,

    config: BrokerConfig,
    pub commission_spec: CommissionSpec,
    pub allow_future_fees: bool,
    pub fractional_shares_trading: bool,
    pub statements_merging_strategy: StatementsMergingStrategy,
}

impl BrokerInfo {
    pub fn get_deposit_commission(&self, country: &Country, assets: CashAssets) -> GenericResult<Cash> {
        let currency = assets.cash.currency;

        let commission = match self.config.deposit_commissions.get(currency) {
            Some(commission_spec) => commission_spec.fixed_amount,
            None if self.type_.jurisdiction() == country.jurisdiction => dec!(0),
            None => return Err!(concat!(
                "Unable to calculate commission for {} deposit to {}: there is no commission ",
                "specification in the configuration file"), currency, self.brief_name),
        };

        Ok(Cash::new(currency, commission))
    }

    pub fn exchanges(&self) -> Vec<Exchange> {
        match self.type_ {
            Broker::Bcs | Broker::Open | Broker::Sber => vec![Exchange::Moex, Exchange::Spb],
            Broker::Tbank => vec![Exchange::Moex, Exchange::Spb, Exchange::Otc],
            Broker::Firstrade => vec![Exchange::Us],
            Broker::InteractiveBrokers => vec![Exchange::Us, Exchange::Other],
        }
    }
}