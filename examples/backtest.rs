use std::io::{self, Write};
use std::process::ExitCode;

use chrono::Duration;
use clap::{Command, Arg, ArgMatches, value_parser};
use easy_logging::LoggingConfig;
use log::error;
use url::Url;

#[macro_use] extern crate investments;

use investments::analysis;
use investments::config::{CliConfig, Config};
use investments::core::{EmptyResult, GenericResult};
use investments::time;

fn main() -> ExitCode {
    let matches = Command::new("backtest")
        .about("Portfolio backtesting tool")
        .help_expected(true)
        .disable_help_subcommand(true)
        .args(Config::args())
        .args([
            Arg::new("url").long("url").short('u')
                .value_name("URL")
                .value_parser(value_parser!(Url))
                .help("VictoriaMetrics URL for metrics backfilling"),

            Arg::new("scrape_period").long("scrape-period").short('s')
                .help("Scrape period (in $number{m|h|d} format)")
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

    let backfilling_url = matches.get_one("url").cloned();
    let scrape_period = matches.get_one("scrape_period").cloned().map(|period| -> GenericResult<Duration> {
        if period < Duration::seconds(1) || period > Duration::days(1) {
            return Err!("Invalid scrape period");
        }
        Ok(period)
    }).transpose()?.unwrap_or(Duration::minutes(1));

    analysis::backtest(&config, backfilling_url.as_ref(), scrape_period)
}