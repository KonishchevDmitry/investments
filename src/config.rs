use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::Duration;
use clap::{Arg, ArgAction, ArgMatches, value_parser};
use serde::Deserialize;
use serde::de::IgnoredAny;
use validator::Validate;

use crate::analysis::performance::config::PerformanceMergingConfig;
use crate::broker_statement::CorporateAction;
use crate::brokers::Broker;
use crate::brokers::config::BrokersConfig;
use crate::cash_flow::config::deserialize_cash_flows;
use crate::core::{GenericResult, EmptyResult};
use crate::deposits::config::DepositConfig;
use crate::instruments::InstrumentInternalIds;
use crate::localities::{self, Country, Jurisdiction};
use crate::metrics::{self, config::MetricsConfig};
use crate::portfolio::config::AssetAllocationConfig;
use crate::quotes::QuotesConfig;
use crate::quotes::alphavantage::AlphaVantageConfig;
use crate::quotes::fcsapi::FcsApiConfig;
use crate::quotes::finnhub::FinnhubConfig;
use crate::quotes::twelvedata::TwelveDataConfig;
use crate::taxes::{self, TaxConfig, TaxExemption, TaxPaymentDay, TaxPaymentDaySpec, TaxRemapping};
use crate::taxes::remapping::TaxRemappingConfig;
use crate::telemetry::TelemetryConfig;
use crate::time;
use crate::types::{Date, Decimal};

pub struct CliConfig {
    pub log_level: log::Level,
    pub config_dir: PathBuf,
    pub cache_expire_time: Option<Duration>,
}

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

    pub fn new<P: AsRef<Path>>(config_dir: P, cache_expire_time: Option<Duration>) -> GenericResult<Config> {
        let config_dir = config_dir.as_ref();

        let config_path = config_dir.join("config.yaml");
        let mut config = Config::load(&config_path).map_err(|e| format!(
            "Error while reading {config_path:?} configuration file: {e}"))?;

        if let Some(cache_expire_time) = cache_expire_time {
            config.cache_expire_time = cache_expire_time;
        }

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

    pub fn args() -> [Arg;3] {[
        Arg::new("config").short('c').long("config")
            .help(format!("Configuration directory path [default: {}]", Self::DEFAULT_CONFIG_DIR_PATH))
            .value_name("PATH")
            .value_parser(value_parser!(PathBuf)),

        Arg::new("verbose").short('v').long("verbose")
            .help("Set verbosity level")
            .action(ArgAction::Count),

        Arg::new("cache_expire_time").short('e').long("cache-expire-time")
            .help("Quote cache expire time (in $number{m|h|d} format)")
            .value_name("DURATION")
            .value_parser(time::parse_duration),
    ]}

    pub fn parse_args(matches: &ArgMatches) -> GenericResult<CliConfig> {
        let log_level = match matches.get_count("verbose") {
            0 => log::Level::Info,
            1 => log::Level::Debug,
            2 => log::Level::Trace,
            _ => return Err!("Invalid verbosity level"),
        };

        let config_dir = matches.get_one("config").cloned().unwrap_or_else(||
            PathBuf::from(shellexpand::tilde(Self::DEFAULT_CONFIG_DIR_PATH).to_string()));

        let cache_expire_time = matches.get_one("cache_expire_time").cloned();

        Ok(CliConfig {
            log_level,
            config_dir,
            cache_expire_time,
        })
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
        if self.quotes.alphavantage.is_none() {
            if let Some(config) = self.alphavantage.take() {
                self.quotes.alphavantage.replace(config);
            }
        }

        if self.quotes.fcsapi.is_none() {
            if let Some(config) = self.fcsapi.take() {
                self.quotes.fcsapi.replace(config);
            }
        }

        if self.quotes.finnhub.is_none() {
            if let Some(config) = self.finnhub.take() {
                self.quotes.finnhub.replace(config);
            }
        }
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

fn default_expire_time() -> Duration {
    Duration::minutes(1)
}