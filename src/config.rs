use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::Read;

use chrono::Duration;
use num_traits::FromPrimitive;
use serde::de::{Deserialize, Deserializer, Error};
use serde_yaml;
use shellexpand;

use core::GenericResult;
use currency::{Cash, CashAssets};
use types::Decimal;
use util::{self, DecimalRestrictions};

#[derive(Deserialize, Debug)]
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
            cache_expire_time: default_expire_time(),

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

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct PortfolioConfig {
    pub name: String,
    pub broker: Broker,
    pub statements: String,
    pub currency: Option<String>,

    #[serde(default)]
    pub assets: Vec<AssetAllocationConfig>,

    #[serde(default, deserialize_with = "deserialize_tax_deductions")]
    pub tax_deductions: Vec<CashAssets>,
}

impl PortfolioConfig {
    pub fn get_stock_symbols(&self) -> HashSet<String> {
        let mut symbols = HashSet::new();

        for asset in &self.assets {
            asset.get_stock_symbols(&mut symbols);
        }

        symbols
    }
}

#[derive(Deserialize, Debug)]
pub struct AssetAllocationConfig {
    pub name: String,
    pub symbol: Option<String>,

    #[serde(deserialize_with = "deserialize_weight")]
    pub weight: Decimal,

    pub assets: Option<Vec<AssetAllocationConfig>>,
}

impl AssetAllocationConfig {
    fn get_stock_symbols(&self, symbols: &mut HashSet<String>) {
        if let Some(ref symbol) = self.symbol {
            symbols.insert(symbol.to_owned());
        }

        if let Some(ref assets) = self.assets {
            for asset in assets {
                asset.get_stock_symbols(symbols);
            }
        }
    }
}

#[derive(Deserialize, Debug)]
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

#[derive(Debug, Clone, Copy)]
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

#[derive(Deserialize, Debug)]
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

fn default_expire_time() -> Duration {
    Duration::minutes(1)
}

fn deserialize_tax_deductions<'de, D>(deserializer: D) -> Result<Vec<CashAssets>, D::Error>
    where D: Deserializer<'de>
{
    let deserialized: HashMap<String, String> = Deserialize::deserialize(deserializer)?;
    let mut tax_deductions = Vec::new();

    for (date, amount) in deserialized.iter() {
        let date = util::parse_date(&date, "%d.%m.%Y").map_err(D::Error::custom)?;
        let amount = util::parse_decimal(&amount, DecimalRestrictions::StrictlyPositive).map_err(|_|
            D::Error::custom(format!("Invalid tax deduction amount: {}", amount)))?;

        tax_deductions.push(CashAssets::new_from_cash(date, Cash::new("RUB", amount)));
    }

    tax_deductions.sort_by_key(|tax_deduction| tax_deduction.date);

    Ok(tax_deductions)
}

fn deserialize_weight<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where D: Deserializer<'de>
{
    let weight: String = Deserialize::deserialize(deserializer)?;
    if !weight.ends_with('%') {
        return Err(D::Error::custom(format!("Invalid weight: {}", weight)));
    }

    let weight = match weight[..weight.len() - 1].parse::<u8>().ok() {
        Some(weight) if weight <= 100 => weight,
        _ => return Err(D::Error::custom(format!("Invalid weight: {}", weight))),
    };

    Ok(Decimal::from_u8(weight).unwrap() / dec!(100))
}