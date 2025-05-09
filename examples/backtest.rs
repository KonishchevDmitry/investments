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

    let instrument = |symbol, exchange| BenchmarkInstrument::new(symbol, exchange);
    let lse = |symbol| instrument(symbol, Exchange::Lse);
    let moex = |symbol| instrument(symbol, Exchange::Moex);

    let (sber, tbank, vtb, blackrock) = ("Sber", "T-Bank", "VTB", "BlackRock");
    let benchmark = |name, provider, instrument| Benchmark::new(name, instrument).with_provider(provider);

    let benchmarks = [
        benchmark("Global stocks", blackrock, lse("SSAC")),
        benchmark("Global corporate bonds", blackrock, lse("IGLA"))
            .then(date!(2018, 5, 15), lse("CRPA"))?,
        benchmark("Global government bonds", blackrock, lse("IGLA")),

        benchmark("Russian stocks", sber, moex("FXRL"))
            .then(date!(2021, 7, 29), moex("SBMX"))?,
        benchmark("Russian stocks", tbank, moex("FXRL"))
            .then(date!(2021, 7, 29), moex("TMOS"))?,
        benchmark("Russian stocks", vtb, moex("FXRL"))
            .then(date!(2021, 7, 29), moex("VTBX").alias("EQMX"))?
            .then_rename(date!(2022, 7, 22), moex("EQMX"))?,

        benchmark("Russian money market", sber, moex("FXRB"))
            .then(date!(2018,  3,  7), moex("FXMM"))?
            .then(date!(2021, 12, 30), moex("SBMM"))?,
        benchmark("Russian money market", tbank, moex("FXRB"))
            .then(date!(2018,  3,  7), moex("FXMM"))?
            .then(date!(2021, 12, 30), moex("SBMM"))?
            .then(date!(2023,  7, 14), moex("TMON"))?,
        benchmark("Russian money market", vtb, moex("FXRB"))
            .then(date!(2018,  3,  7), moex("FXMM"))?
            .then(date!(2021, 12, 30), moex("VTBM"))?
            .then_rename(date!(2022, 7, 22), moex("LQDT"))?,

        benchmark("Russian government bonds", sber, moex("FXRB"))
            .then(date!(2019,  1, 25), moex("SBGB"))?,
        benchmark("Russian government bonds", tbank, moex("FXRB"))
            .then(date!(2019,  1, 25), moex("SBGB"))?
            .then(date!(2024, 12, 17), moex("TOFZ"))?,

        benchmark("Russian corporate bonds", sber, moex("FXRB"))
            .then(date!(2020,  5, 20), moex("SBRB"))?,
        benchmark("Russian corporate bonds", tbank, moex("FXRB"))
            .then(date!(2020,  5, 20), moex("SBRB"))?
            .then(date!(2021,  8,  6), moex("TBRU"))?,
        benchmark("Russian corporate bonds", vtb, moex("FXRB"))
            .then(date!(2020,  5, 20), moex("SBRB"))?
            .then(date!(2021,  8,  6), moex("VTBB"))?
            .then_rename(date!(2022, 7, 22), moex("OBLG"))?,

        benchmark("Russian corporate eurobonds", sber, moex("FXRU"))
            .then(date!(2020,  9, 24), moex("SBCB"))?
            .then(date!(2022,  1, 25), moex("SBMM"))? // SBCB was frozen for this period. Ideally we need some stub only for new deposits
            .then(date!(2023, 12, 15), moex("SBCB"))?, // The open price is equal to close price of previous SBCB interval
        benchmark("Russian corporate eurobonds", tbank, moex("FXRU"))
            .then(date!(2020,  9, 24), moex("SBCB"))?
            .then(date!(2022,  1, 25), moex("SBMM"))? // SBCB was frozen for this period. Ideally we need some stub only for new deposits
            .then(date!(2023, 12, 15), moex("SBCB"))? // The open price is equal to close price of previous SBCB interval
            .then(date!(2024,  4,  1), moex("TLCB"))?,

        benchmark("Gold", sber, moex("FXRU"))
            .then(date!(2018,  3,  7), moex("FXGD"))?
            .then(date!(2020,  7, 15), moex("VTBG"))?
            .then_rename(date!(2022, 7, 22), moex("GOLD"))?
            .then(date!(2022, 11, 21), moex("SBGD"))?,
        benchmark("Gold", tbank, moex("FXRU"))
            .then(date!(2018,  3,  7), moex("FXGD"))?
            .then(date!(2020,  7, 15), moex("VTBG"))?
            .then_rename(date!(2022, 7, 22), moex("GOLD"))?
            .then(date!(2024, 11,  5), moex("TGLD"))?,
        benchmark("Gold", vtb, moex("FXRU"))
            .then(date!(2018,  3,  7), moex("FXGD"))?
            .then(date!(2020,  7, 15), moex("VTBG"))?
            .then_rename(date!(2022, 7, 22), moex("GOLD"))?,
    ];

    analysis::backtest(&config, portfolio.as_deref(), Some(&benchmarks), backfilling_config, Some(method))?;

    Ok(())
}