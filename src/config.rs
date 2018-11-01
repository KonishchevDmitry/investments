use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use serde_yaml;

use core::GenericResult;
use types::Decimal;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(skip)]
    pub db_path: String,

    pub alphavantage: AlphaVantageConfig,
    pub interactive_brokers: BrokerConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AlphaVantageConfig {
    pub api_key: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BrokerConfig {
    pub deposit_commissions: HashMap<String, TransactionCommissionSpec>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TransactionCommissionSpec {
    pub fixed_amount: Decimal,
}

impl Config {
    #[cfg(test)]
    pub fn mock() -> Config {
        Config {
            db_path: "/mock".to_owned(),
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
    Ok(serde_yaml::from_slice(&data)?)
}