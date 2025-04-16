use std::collections::{HashSet, HashMap, BTreeMap};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chrono::Duration;
use clap::{Arg, ArgAction, ArgMatches, value_parser};
use log::Level;
use serde::Deserialize;
use serde::de::{Deserializer, IgnoredAny, Error};
use validator::Validate;

use crate::analysis::config::PerformanceMergingConfig;
use crate::broker_statement::CorporateAction;
use crate::brokers::Broker;
use crate::core::{GenericResult, EmptyResult};
use crate::formatting;
use crate::instruments::InstrumentInternalIds;
use crate::localities::{self, Country, Jurisdiction};
use crate::metrics::{self, config::MetricsConfig};
use crate::quotes::QuotesConfig;
use crate::quotes::alphavantage::AlphaVantageConfig;
use crate::quotes::fcsapi::FcsApiConfig;
use crate::quotes::finnhub::FinnhubConfig;
use crate::quotes::tbank::TbankApiConfig;
use crate::quotes::twelvedata::TwelveDataConfig;
use crate::taxes::{self, TaxConfig, TaxExemption, TaxPaymentDay, TaxPaymentDaySpec, TaxRemapping};
use crate::telemetry::TelemetryConfig;
use crate::time::{self, deserialize_date};
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(skip)]
    pub db_path: String,
    #[serde(skip, default = "default_expire_time")]
    pub cache_expire_time: Duration,

    #[serde(default)]
    pub deposits: Vec<DepositConfig>,
    pub notify_deposit_closing_days: Option<u32>,

    #[serde(default)]
    pub portfolios: Vec<PortfolioConfig>,
    pub brokers: Option<BrokersConfig>,
    #[serde(default)]
    pub taxes: TaxConfig,

    #[validate(nested)]
    #[serde(default)]
    pub quotes: QuotesConfig,
    #[validate(nested)]
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub telemetry: TelemetryConfig,

    // Deprecated
    pub alphavantage: Option<AlphaVantageConfig>,
    pub fcsapi: Option<FcsApiConfig>,
    pub finnhub: Option<FinnhubConfig>,
    pub twelvedata: Option<TwelveDataConfig>,

    #[serde(default, rename="anchors")]
    _anchors: IgnoredAny,
}

impl Config {
    const DEFAULT_CONFIG_DIR_PATH: &str = "~/.investments";

    pub fn new<P: AsRef<Path>>(config_dir: P) -> GenericResult<Config> {
        let config_dir = config_dir.as_ref();

        let config_path = config_dir.join("config.yaml");
        let mut config = Config::load(&config_path).map_err(|e| format!(
            "Error while reading {config_path:?} configuration file: {e}"))?;

        config_dir.join("db.sqlite").to_str()
            .ok_or_else(|| format!("Invalid configuration directory path: {config_dir:?}"))?
            .clone_into(&mut config.db_path);

        Ok(config)
    }

    #[cfg(test)]
    pub fn mock() -> Config {
        Config {
            db_path: s!("/mock"),
            cache_expire_time: default_expire_time(),

            deposits: Vec::new(),
            notify_deposit_closing_days: None,

            portfolios: Vec::new(),
            brokers: None,
            taxes: Default::default(),

            quotes: Default::default(),
            metrics: Default::default(),

            alphavantage: None,
            fcsapi: None,
            finnhub: None,
            twelvedata: None,
            telemetry: Default::default(),

            _anchors: Default::default(),
        }
    }

    pub fn args() -> [Arg;2] {[
        Arg::new("verbose").short('v').long("verbose")
            .help("Set verbosity level")
            .action(ArgAction::Count),

        Arg::new("config").short('c').long("config")
            .help(format!("Configuration directory path [default: {}]", Self::DEFAULT_CONFIG_DIR_PATH))
            .value_name("PATH")
            .value_parser(value_parser!(PathBuf)),
    ]}

    pub fn parse_args(matches: &ArgMatches) -> GenericResult<(Level, PathBuf)> {
        let log_level = match matches.get_count("verbose") {
            0 => log::Level::Info,
            1 => log::Level::Debug,
            2 => log::Level::Trace,
            _ => return Err!("Invalid verbosity level"),
        };

        let config_dir = matches.get_one("config").cloned().unwrap_or_else(||
            PathBuf::from(shellexpand::tilde(Self::DEFAULT_CONFIG_DIR_PATH).to_string()));

        Ok((log_level, config_dir))
    }

    pub fn get_tax_country(&self) -> Country {
        localities::russia(&self.taxes)
    }

    pub fn get_portfolio(&self, name: &str) -> GenericResult<&PortfolioConfig> {
        for portfolio in &self.portfolios {
            if portfolio.name == name {
                return Ok(portfolio)
            }
        }

        Err!("{:?} portfolio is not defined in the configuration file", name)
    }

    fn load(path: &Path) -> GenericResult<Config> {
        let mut config: Config = Config::read(path)?;

        config.validate()?;
        config.move_deprecated_settings();

        let mut portfolio_names = HashSet::new();

        for portfolio in &mut config.portfolios {
            if portfolio.name == metrics::PORTFOLIO_LABEL_ALL {
                return Err!("Invalid portfolio name: {:?}. The name is reserved", portfolio.name);
            } else if !portfolio_names.insert(portfolio.name.clone()) {
                return Err!("Duplicate portfolio name: {:?}", portfolio.name);
            }

            portfolio.statements = portfolio.statements.as_ref().map(|path|
                shellexpand::tilde(path).to_string());

            portfolio.validate().map_err(|e| format!(
                "{:?} portfolio: {}", portfolio.name, e))?;
        }

        for deposit in &config.deposits {
            deposit.validate()?;
        }

        config.metrics.validate_inner(&portfolio_names)?;

        Ok(config)
    }

    fn read(path: &Path) -> GenericResult<Config> {
        let mut data = Vec::new();
        File::open(path)?.read_to_end(&mut data)?;

        {
            // yaml-rust doesn't support merge key (https://github.com/chyh1990/yaml-rust/issues/68)

            use yaml_merge_keys::serde_yaml::{self as yaml, Value};

            let value: Value = yaml::from_slice(&data)?;
            let merged = yaml_merge_keys::merge_keys_serde(value.clone())?;
            if merged == value {
                return Ok(serde_yaml::from_slice(&data)?);
            }

            data.clear();
            yaml::to_writer(&mut data, &merged)?
        }

        Ok(serde_yaml::from_slice(&data).map_err(|err| {
            // To not confuse user with changed positions
            if let Some(message) = err.location().and_then(|location| {
                let message = err.to_string();
                let suffix = format!(" at line {} column {}", location.line(), location.column());
                message.strip_suffix(&suffix).map(ToOwned::to_owned)
            }) {
                return message;
            }

            err.to_string()
        })?)
    }

    fn move_deprecated_settings(&mut self) {
        if self.quotes.fcsapi.is_none() {
            if let Some(fcsapi) = self.fcsapi.take() {
                self.quotes.fcsapi.replace(fcsapi);
            }
        }

        if self.quotes.finnhub.is_none() {
            if let Some(finnhub) = self.finnhub.take() {
                self.quotes.finnhub.replace(finnhub);
            }
        }
    }
}

// FIXME(konishchev): Refactor all below

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DepositConfig {
    pub name: String,

    #[serde(deserialize_with = "deserialize_date")]
    pub open_date: Date,
    #[serde(deserialize_with = "deserialize_date")]
    pub close_date: Date,

    #[serde(default)]
    pub currency: Option<String>,
    pub amount: Decimal,
    pub interest: Decimal,
    #[serde(default)]
    pub capitalization: bool,
    #[serde(default, deserialize_with = "deserialize_cash_flows")]
    pub contributions: Vec<(Date, Decimal)>,
}

impl DepositConfig {
    fn validate(&self) -> EmptyResult {
        if self.open_date > self.close_date {
            return Err!(
                "Invalid {:?} deposit dates: {} -> {}",
                self.name, formatting::format_date(self.open_date),
                formatting::format_date(self.close_date));
        }

        for &(date, _amount) in &self.contributions {
            if date < self.open_date || date > self.close_date {
                return Err!(
                    "Invalid {:?} deposit contribution date: {}",
                    self.name, formatting::format_date(date));
            }
        }

        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortfolioConfig {
    pub name: String,
    pub broker: Broker,
    pub plan: Option<String>,

    pub statements: Option<String>,
    #[serde(default)]
    pub symbol_remapping: HashMap<String, String>,
    #[serde(default, deserialize_with = "InstrumentInternalIds::deserialize")]
    pub instrument_internal_ids: InstrumentInternalIds,
    #[serde(default)]
    pub instrument_names: HashMap<String, String>,
    #[serde(default)]
    tax_remapping: Vec<TaxRemappingConfig>,
    #[serde(default)]
    pub corporate_actions: Vec<CorporateAction>,

    pub currency: Option<String>,
    pub min_trade_volume: Option<Decimal>,
    pub min_cash_assets: Option<Decimal>,
    pub restrict_buying: Option<bool>,
    pub restrict_selling: Option<bool>,

    #[serde(default)]
    pub merge_performance: PerformanceMergingConfig,

    #[serde(default)]
    pub assets: Vec<AssetAllocationConfig>,

    #[serde(default, rename = "tax_payment_day", deserialize_with = "TaxPaymentDaySpec::deserialize")]
    tax_payment_day_spec: TaxPaymentDaySpec,

    #[serde(default)]
    pub tax_exemptions: Vec<TaxExemption>,

    #[serde(default, deserialize_with = "deserialize_cash_flows")]
    pub tax_deductions: Vec<(Date, Decimal)>,
}

impl PortfolioConfig {
    pub fn currency(&self) -> &str {
        self.currency.as_deref().unwrap_or_else(|| self.broker.jurisdiction().traits().currency)
    }

    pub fn statements_path(&self) -> GenericResult<&str> {
        Ok(self.statements.as_ref().ok_or("Broker statements path is not specified in the portfolio's config")?)
    }

    pub fn get_stock_symbols(&self) -> HashSet<String> {
        let mut symbols = HashSet::new();

        for asset in &self.assets {
            asset.get_stock_symbols(&mut symbols);
        }

        symbols
    }

    pub fn tax_payment_day(&self) -> TaxPaymentDay {
        TaxPaymentDay::new(self.broker.jurisdiction(), self.tax_payment_day_spec)
    }

    pub fn get_tax_remapping(&self) -> GenericResult<TaxRemapping> {
        let mut remapping = TaxRemapping::new();

        for config in &self.tax_remapping {
            remapping.add(config.date, &config.description, config.to_date)?;
        }

        Ok(remapping)
    }

    pub fn close_date() -> Date {
        time::today()
    }

    fn validate(&self) -> EmptyResult {
        let currency = self.currency();

        match currency {
            "RUB" | "USD" => (),
            _ => return Err!("Unsupported portfolio currency: {currency}"),
        }

        for (symbol, mapping) in &self.symbol_remapping {
            if self.symbol_remapping.contains_key(mapping) {
                return Err!("Invalid symbol remapping configuration: Recursive {} symbol", symbol);
            }
        }

        if
            matches!(self.tax_payment_day_spec, TaxPaymentDaySpec::OnClose(_)) &&
            self.broker.jurisdiction() != Jurisdiction::Russia
        {
            return Err!("On close tax payment date is only available for brokers with Russia jurisdiction")
        }

        taxes::validate_tax_exemptions(self.broker, &self.tax_exemptions)?;

        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaxRemappingConfig {
    #[serde(deserialize_with = "deserialize_date")]
    pub date: Date,
    pub description: String,
    #[serde(deserialize_with = "deserialize_date")]
    pub to_date: Date,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetAllocationConfig {
    pub name: String,
    pub symbol: Option<String>,

    #[serde(deserialize_with = "deserialize_weight")]
    pub weight: Decimal,
    pub restrict_buying: Option<bool>,
    pub restrict_selling: Option<bool>,

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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrokersConfig {
    pub bcs: Option<BrokerConfig>,
    pub firstrade: Option<BrokerConfig>,
    pub interactive_brokers: Option<BrokerConfig>,
    pub open_broker: Option<BrokerConfig>,
    pub sber: Option<BrokerConfig>,
    #[serde(alias = "tinkoff")]
    pub tbank: Option<TbankConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TbankConfig {
    #[serde(flatten)]
    pub broker: Option<BrokerConfig>,
    #[serde(flatten)]
    pub api: Option<TbankApiConfig>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
pub struct BrokerConfig {
    pub deposit_commissions: HashMap<String, TransactionCommissionSpec>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TaxRates {
    #[serde(default)]
    pub trading: BTreeMap<i32, Decimal>,
    #[serde(default)]
    pub dividends: BTreeMap<i32, Decimal>,
    #[serde(default)]
    pub interest: BTreeMap<i32, Decimal>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TransactionCommissionSpec {
    pub fixed_amount: Decimal,
}

fn default_expire_time() -> Duration {
    Duration::minutes(1)
}

fn deserialize_cash_flows<'de, D>(deserializer: D) -> Result<Vec<(Date, Decimal)>, D::Error>
    where D: Deserializer<'de>
{
    let deserialized: HashMap<String, Decimal> = Deserialize::deserialize(deserializer)?;
    let mut cash_flows = Vec::new();

    for (date, amount) in deserialized {
        let date = time::parse_user_date(&date).map_err(D::Error::custom)?;
        let amount = util::validate_decimal(amount, DecimalRestrictions::StrictlyPositive).map_err(|_|
            D::Error::custom(format!("Invalid amount: {:?}", amount)))?;

        cash_flows.push((date, amount));
    }

    cash_flows.sort_by_key(|cash_flow| cash_flow.0);

    Ok(cash_flows)
}

fn deserialize_weight<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
    where D: Deserializer<'de>
{
    let weight: String = Deserialize::deserialize(deserializer)?;

    let weight = Some(weight.as_str())
        .and_then(|weight| weight.strip_suffix('%'))
        .and_then(|weight| Decimal::from_str(weight).ok())
        .and_then(|weight| {
            if weight.is_sign_positive() && util::decimal_precision(weight) <= 2 && weight <= dec!(100) {
                Some(weight.normalize())
            } else {
                None
            }
        }).ok_or_else(|| D::Error::custom(format!("Invalid weight: {}", weight)))?;

    Ok(weight / dec!(100))
}