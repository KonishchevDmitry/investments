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
use investments::analysis::backtesting::{Benchmark, BenchmarkInstrument, BenchmarkPerformanceType};
use investments::config::{CliConfig, Config};
use investments::core::{EmptyResult, GenericResult};
use investments::exchanges::Exchange;
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

    let instrument = |symbol: &str| BenchmarkInstrument::new(symbol, Exchange::Moex);

    let (sber, tbank, vtb) = ("Sber", "T-Bank", "VTB");
    let benchmark = |name: &str, provider: &str, symbol: &str| Benchmark::new(name, instrument(symbol)).with_provider(provider);

    let benchmarks = [
        benchmark("Russian stocks", sber, "FXRL")
            .then(date!(2021, 7, 29), instrument("SBMX"))?,
        benchmark("Russian stocks", tbank, "FXRL")
            .then(date!(2021, 7, 29), instrument("TMOS"))?,
        benchmark("Russian stocks", vtb, "FXRL")
            .then(date!(2021, 7, 29), instrument("VTBX").alias("EQMX"))?
            .then_rename(date!(2022, 7, 22), instrument("EQMX"))?,

        benchmark("Russian money market", sber, "FXRB")
            .then(date!(2018,  3,  7), instrument("FXMM"))?
            .then(date!(2021, 12, 30), instrument("SBMM"))?,
        benchmark("Russian money market", tbank, "FXRB")
            .then(date!(2018,  3,  7), instrument("FXMM"))?
            .then(date!(2021, 12, 30), instrument("SBMM"))?
            .then(date!(2023,  7, 14), instrument("TMON"))?,
        benchmark("Russian money market", vtb, "FXRB")
            .then(date!(2018,  3,  7), instrument("FXMM"))?
            .then(date!(2021, 12, 30), instrument("VTBM"))?
            .then_rename(date!(2022, 7, 22), instrument("LQDT"))?,

        benchmark("Russian government bonds", sber, "FXRB")
            .then(date!(2019,  1, 25), instrument("SBGB"))?,
        benchmark("Russian government bonds", tbank, "FXRB")
            .then(date!(2019,  1, 25), instrument("SBGB"))?
            .then(date!(2024, 12, 17), instrument("TOFZ"))?,

        benchmark("Russian corporate bonds", sber, "FXRB")
            .then(date!(2020,  5, 20), instrument("SBRB"))?,
        benchmark("Russian corporate bonds", tbank, "FXRB")
            .then(date!(2020,  5, 20), instrument("SBRB"))?
            .then(date!(2021,  8,  6), instrument("TBRU"))?,
        benchmark("Russian corporate bonds", vtb, "FXRB")
            .then(date!(2020,  5, 20), instrument("SBRB"))?
            .then(date!(2021,  8,  6), instrument("VTBB"))?
            .then_rename(date!(2022, 7, 22), instrument("OBLG"))?,

        benchmark("Russian corporate eurobonds", sber, "FXRU")
            .then(date!(2020,  9, 24), instrument("SBCB"))?
            .then(date!(2022,  1, 25), instrument("SBMM"))? // SBCB was frozen for this period. Ideally we need some stub only for new deposits
            .then(date!(2023, 12, 15), instrument("SBCB"))?, // The open price is equal to close price of previous SBCB interval
        benchmark("Russian corporate eurobonds", tbank, "FXRU")
            .then(date!(2020,  9, 24), instrument("SBCB"))?
            .then(date!(2022,  1, 25), instrument("SBMM"))? // SBCB was frozen for this period. Ideally we need some stub only for new deposits
            .then(date!(2023, 12, 15), instrument("SBCB"))? // The open price is equal to close price of previous SBCB interval
            .then(date!(2024,  4,  1), instrument("TLCB"))?,

        benchmark("Gold", sber, "FXRU")
            .then(date!(2018,  3,  7), instrument("FXGD"))?
            .then(date!(2020,  7, 15), instrument("VTBG"))?
            .then_rename(date!(2022, 7, 22), instrument("GOLD"))?
            .then(date!(2022, 11, 21), instrument("SBGD"))?,
        benchmark("Gold", tbank, "FXRU")
            .then(date!(2018,  3,  7), instrument("FXGD"))?
            .then(date!(2020,  7, 15), instrument("VTBG"))?
            .then_rename(date!(2022, 7, 22), instrument("GOLD"))?
            .then(date!(2024, 11,  5), instrument("TGLD"))?,
        benchmark("Gold", vtb, "FXRU")
            .then(date!(2018,  3,  7), instrument("FXGD"))?
            .then(date!(2020,  7, 15), instrument("VTBG"))?
            .then_rename(date!(2022, 7, 22), instrument("GOLD"))?,
    ];

    // FIXME(konishchev): Drop it
    analysis::backtest(&config, portfolio.as_deref(), Some(&benchmarks[..3]), backfilling_config, Some(method))?;

    Ok(())
}