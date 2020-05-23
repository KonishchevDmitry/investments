mod plans;

use matches::matches;
use serde::Deserialize;
use serde::de::{Deserializer, Error as _};

use crate::commissions::CommissionSpec;
use crate::config::{Config, BrokersConfig, BrokerConfig};
use crate::core::GenericResult;
use crate::currency::CashAssets;
use crate::types::Decimal;

#[derive(Debug, Clone, Copy)]
pub enum Broker {
    Bcs,
    InteractiveBrokers,
    OpenBroker,
    Tinkoff,
}

impl Broker {
    pub fn get_info(self, config: &Config, _plan: Option<&String>) -> GenericResult<BrokerInfo> {
        let config = config.brokers.as_ref()
            .and_then(|brokers| self.get_config(brokers))
            .ok_or_else(|| format!(
                "{} configuration is not set in the configuration file", self.get_name()))?
            .clone();

        Ok(BrokerInfo {
            type_: self,
            name: self.get_name(),
            config: config,
            commission_spec: self.get_commission_spec(),
            allow_sparse_broker_statements: matches!(self, Broker::Bcs),
        })
    }

    fn get_name(self) -> &'static str {
        match self {
            Broker::Bcs => "ООО «Компания БКС»",
            Broker::InteractiveBrokers => "Interactive Brokers LLC",
            Broker::OpenBroker => "АО «Открытие Брокер»",
            Broker::Tinkoff => "АО «Тинькофф Банк»",
        }
    }

    fn get_config(self, config: &BrokersConfig) -> Option<&BrokerConfig> {
        match self {
            Broker::Bcs => &config.bcs,
            Broker::InteractiveBrokers => &config.interactive_brokers,
            Broker::OpenBroker => &config.open_broker,
            Broker::Tinkoff => &config.tinkoff,
        }.as_ref()
    }

    // FIXME(konishchev): Configurable commissions support
    fn get_commission_spec(self) -> CommissionSpec {
        match self {
            Broker::Bcs => plans::bcs::professional(),
            Broker::InteractiveBrokers => plans::ib::fixed(),
            Broker::OpenBroker => plans::open::iia(),
            Broker::Tinkoff => plans::tinkoff::trader(),
        }
    }
}

impl<'de> Deserialize<'de> for Broker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;

        Ok(match value.as_str() {
            "bcs" => Broker::Bcs,
            "interactive-brokers" => Broker::InteractiveBrokers,
            "open-broker" => Broker::OpenBroker,
            "tinkoff" => Broker::Tinkoff,

            _ => return Err(D::Error::unknown_variant(&value, &[
                "bcs", "interactive-brokers", "open-broker", "tinkoff",
            ])),
        })
    }
}

#[derive(Debug, Clone)]
pub struct BrokerInfo {
    pub type_: Broker,
    pub name: &'static str,
    config: BrokerConfig,
    pub commission_spec: CommissionSpec,
    pub allow_sparse_broker_statements: bool,
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