use std::io::{self, Write};
use std::process::ExitCode;
use std::str::FromStr;

use chrono::Duration;
use clap::{Command, Arg, ArgMatches, value_parser};
use clap::builder::NonEmptyStringValueParser;
use easy_logging::LoggingConfig;
use itertools::Itertools;
use log::error;
use strum::IntoEnumIterator;
use url::Url;

#[macro_use] extern crate investments;

use investments::analysis;
use investments::analysis::backtesting::BenchmarkPerformanceType;
use investments::config::{CliConfig, Config};
use investments::core::{EmptyResult, GenericResult};
use investments::metrics::backfilling::BackfillingConfig;
use investments::time;

fn main() -> ExitCode {
    let matches = Command::new("backtest")
        .about("Portfolio backtesting tool")
        .help_expected(true)
        .disable_help_subcommand(true)
        .args(Config::args())
        .args([
            Arg::new("portfolio")
                .help("Portfolio name (omit to show an aggregated result for all portfolios)")
                .value_name("PORTFOLIO")
                .value_parser(NonEmptyStringValueParser::new())
                .conflicts_with("url"),

            Arg::new("method").short('m').long("method")
                .help(format!("Performance analysis method ({})", BenchmarkPerformanceType::iter().map(|method| {
                        Into::<&'static str>::into(method)
                }).join(", ")))
                .value_name("METHOD")
                .value_parser(BenchmarkPerformanceType::from_str)
                .default_value(Into::<&'static str>::into(BenchmarkPerformanceType::Virtual)),

            Arg::new("url").long("url").short('u')
                .help("VictoriaMetrics URL for metrics backfilling")
                .value_name("URL")
                .value_parser(value_parser!(Url)),

            Arg::new("scrape_interval").long("scrape-interval").short('s')
                .help("Scrape interval (in $number{m|h|d} format)")
                .value_name("DURATION")
                .value_parser(time::parse_duration),
        ])
        .get_matches();

    let cli_config = match Config::parse_args(&matches) {
        Ok(cli_config) => cli_config,
        Err(err) => {
            let _ = writeln!(io::stderr(), "{err}.");
            return ExitCode::FAILURE;
        },
    };

    let logging = LoggingConfig::new(module_path!(), cli_config.log_level)
        .level_for("investments", cli_config.log_level);

    if let Err(err) = logging.build() {
        let _ = writeln!(io::stderr(), "Failed to initialize the logging: {err}.");
        return ExitCode::FAILURE;
    }

    if let Err(err) = run(cli_config, &matches) {
        error!("{err}.");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

pub fn run(cli_config: CliConfig, matches: &ArgMatches) -> EmptyResult {
    let config = Config::new(&cli_config.config_dir, cli_config.cache_expire_time)?;

    let portfolio = matches.get_one::<String>("portfolio").cloned();
    let method = matches.get_one("method").cloned().unwrap();

    let scrape_interval = matches.get_one("scrape_interval").cloned().map(|interval| -> GenericResult<Duration> {
        if interval < Duration::seconds(1) || interval > Duration::days(1) {
            return Err!("Invalid scrape interval");
        }
        Ok(interval)
    }).transpose()?.unwrap_or(Duration::minutes(1));

    let backfilling_config = matches.get_one("url").cloned().map(|url| {
        BackfillingConfig {
            url,
            scrape_interval,
        }
    });

    analysis::backtest(&config, portfolio.as_deref(), true, backfilling_config, Some(method))?;

    Ok(())
}