use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::Read;

use chrono::Duration;
use serde::de::{Deserialize, Deserializer, Error};
use serde_yaml;
use shellexpand;

use core::GenericResult;
use types::Decimal;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(skip)]
    pub db_path: String,

    #[serde(skip, default = "default_expire_time")]
    pub cache_expire_time: Duration,

    pub portfolios: Vec<PortfolioConfig>,
    pub brokers: BrokersConfig,

    pub alphavantage: AlphaVantageConfig,
}

impl Config {
    #[cfg(test)]
    pub fn mock() -> Config {
        Config {
            db_path: "/mock".to_owned(),
            cache_expire_time: default_expire_time,

            portfolios: Vec::new(),
            brokers: BrokersConfig::mock(),
            alphavantage: AlphaVantageConfig {
                api_key: s!("mock"),
            },
        }
    }

    pub fn get_portfolio(&self, name: &str) -> GenericResult<&PortfolioConfig> {
        for portfolio in &self.portfolios {
            if portfolio.name == name {
                return Ok(portfolio)
            }
        }

        Err!("{:?} portfolio is not defined in the configuration file", name)
    }
}

fn default_expire_time() -> Duration {
    Duration::minutes(1)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortfolioConfig {
    pub name: String,
    pub broker: Broker,
    pub statements: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokersConfig {
    pub interactive_brokers: Option<BrokerConfig>,
    pub open_broker: Option<BrokerConfig>,
}

impl BrokersConfig {
    #[cfg(test)]
    pub fn mock() -> BrokersConfig {
        BrokersConfig {
            interactive_brokers: Some(BrokerConfig::mock()),
            open_broker: Some(BrokerConfig::mock()),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct BrokerConfig {
    pub deposit_commissions: HashMap<String, TransactionCommissionSpec>,
}

impl BrokerConfig {
    #[cfg(test)]
    pub fn mock() -> BrokerConfig {
        BrokerConfig {
            deposit_commissions: HashMap::new(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TransactionCommissionSpec {
    pub fixed_amount: Decimal,
}

#[derive(Clone, Copy)]
pub enum Broker {
    InteractiveBrokers,
    OpenBroker,
}

impl<'de> Deserialize<'de> for Broker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;

        Ok(match value.as_str() {
            "interactive-brokers" => Broker::InteractiveBrokers,
            "open-broker" => Broker::OpenBroker,

            _ => return Err(D::Error::unknown_variant(&value, &[
                "interactive-brokers", "open-broker",
            ])),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaVantageConfig {
    pub api_key: String,
}

pub fn load_config(path: &str) -> GenericResult<Config> {
    let mut data = Vec::new();
    File::open(path)?.read_to_end(&mut data)?;

    let mut config: Config = serde_yaml::from_slice(&data)?;

    {
        let mut portfolio_names = HashSet::new();

        for portfolio in &config.portfolios {
            if !portfolio_names.insert(&portfolio.name) {
                return Err!("Duplicate portfolio name: {:?}", portfolio.name);
            }
        }
    }

    for portfolio in &mut config.portfolios {
        portfolio.statements = shellexpand::tilde(&portfolio.statements).to_string();
    }

    Ok(config)
}