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
    pub alphavantage: AlphaVantageConfig,
    pub interactive_brokers: BrokerConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortfolioConfig {
    pub name: String,
    pub broker: Broker,
    pub statement: String,
}

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

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct BrokerConfig {
    pub deposit_commissions: HashMap<String, TransactionCommissionSpec>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct TransactionCommissionSpec {
    pub fixed_amount: Decimal,
}

impl Config {
    #[cfg(test)]
    pub fn mock() -> Config {
        Config {
            db_path: "/mock".to_owned(),
            portfolios: Vec::new(),
            alphavantage: AlphaVantageConfig {
                api_key: s!("mock"),
            },
            interactive_brokers: BrokerConfig {
                deposit_commissions: HashMap::new(),
            },
        }
    }
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