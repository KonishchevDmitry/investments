use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::Read;

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

    pub portfolios: Vec<PortfolioConfig>,
    pub interactive_brokers: BrokerConfig,
    pub alphavantage: AlphaVantageConfig,
}

impl Config {
    #[cfg(test)]
    pub fn mock() -> Config {
        Config {
            db_path: "/mock".to_owned(),
            portfolios: Vec::new(),
            interactive_brokers: BrokerConfig::mock(),
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortfolioConfig {
    pub name: String,
    pub broker: Broker,
    pub statement: String,
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
}

impl<'de> Deserialize<'de> for Broker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let value = String::deserialize(deserializer)?;

        Ok(match value.as_str() {
            "interactive-brokers" => Broker::InteractiveBrokers,
            _ => return Err(D::Error::unknown_variant(&value, &["interactive-brokers"])),
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
        portfolio.statement = shellexpand::tilde(&portfolio.statement).to_string();
    }

    Ok(config)
}