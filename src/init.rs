use std::io::{self, Write};
use std::path::Path;
use std::process;
use std::str::FromStr;

use clap::{App, Arg, AppSettings, SubCommand, ArgMatches};
use easy_logging;
use log::{self, debug, error};
use shellexpand;

use crate::config::{Config, load_config};
use crate::core::GenericResult;
use crate::types::{Date, Decimal};
use crate::util;

pub enum Action {
    Analyse(String),

    Sync(String),
    Buy(String, u32, String, Decimal),
    Sell(String, u32, String, Decimal),
    SetCashAssets(String, Decimal),

    Show {
        name: String,
        flat: bool,
    },
    Rebalance {
        name: String,
        flat: bool,
    },

    TaxStatement {
        name: String,
        year: i32,
        tax_statement_path: Option<String>,
    },
}

pub fn initialize() -> (Action, Config) {
    let default_config_dir_path = "~/.investments";

    // FIXME: Stock selling emulation (taxes, profit)
    let matches = App::new("Investments")
        .about("\nHelps you with managing your investments")
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("PATH")
            .help(&format!("Configuration directory path [default: {}]", default_config_dir_path))
            .takes_value(true))
        .arg(Arg::with_name("cache_expire_time")
            .short("e")
            .long("cache-expire-time")
            .value_name("DURATION")
            .help("Quote cache expire time (in $number{m|h|d} format)")
            .takes_value(true))
        .arg(Arg::with_name("verbose")
            .short("v")
            .long("verbose")
            .multiple(true)
            .help("Sets the level of verbosity"))
        .subcommand(SubCommand::with_name("analyse")
            .about("Analyze portfolio performance")
            .long_about(concat!(
                "\nCalculates average rate of return from cash investments by comparing portfolio ",
                "performance to performance of a bank deposit with exactly the same investments ",
                "and monthly capitalization."))
            .arg(portfolio::arg()))
        .subcommand(SubCommand::with_name("show")
            .about("Show portfolio's asset allocation")
            .arg(Arg::with_name("flat")
                .short("f")
                .long("flat")
                .help("Flat view"))
            .arg(portfolio::arg()))
        .subcommand(SubCommand::with_name("sync")
            .about("Sync portfolio with broker statement")
            .arg(portfolio::arg()))
        .subcommand(SubCommand::with_name("buy")
            .about("Add the specified stock shares to the portfolio")
            .arg(portfolio::arg())
            .arg(shares::arg())
            .arg(symbol::arg())
            .arg(cash_assets::arg()))
        .subcommand(SubCommand::with_name("sell")
            .about("Remove the specified stock shares from the portfolio")
            .arg(portfolio::arg())
            .arg(shares::arg())
            .arg(symbol::arg())
            .arg(cash_assets::arg()))
        .subcommand(SubCommand::with_name("cash")
            .about("Set current cash assets")
            .arg(portfolio::arg())
            .arg(cash_assets::arg()))
        .subcommand(SubCommand::with_name("rebalance")
            .about("Rebalance the portfolio according to the asset allocation configuration")
            .arg(Arg::with_name("flat")
                .short("f")
                .long("flat")
                .help("Flat view"))
            .arg(portfolio::arg()))
        .subcommand(SubCommand::with_name("tax-statement")
            .about("Generate tax statement")
            .long_about(concat!(
                "\nReads broker statements and alters *.dcX file (created by Russian tax program ",
                "named Декларация) by adding all required information about income from paid ",
                "dividends.\n",
                "\nIf tax statement file is not specified only outputs the data which is going to ",
                "be declared."))
            .arg(portfolio::arg())
            .arg(Arg::with_name("YEAR")
                .help("Year to generate the statement for")
                .required(true))
            .arg(Arg::with_name("TAX_STATEMENT")
                .help("Path to tax statement *.dcX file")))
        .global_setting(AppSettings::DisableVersion)
        .global_setting(AppSettings::DisableHelpSubcommand)
        .global_setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .get_matches();

    let log_level = match matches.occurrences_of("verbose") {
        0 => log::Level::Info,
        1 => log::Level::Debug,
        2 => log::Level::Trace,
        _ => {
            let _ = writeln!(io::stderr(), "Invalid verbosity level.");
            process::exit(1);
        }
    };

    if let Err(err) = easy_logging::init(module_path!().split("::").next().unwrap(), log_level) {
        let _ = writeln!(io::stderr(), "Failed to initialize the logging: {}.", err);
        process::exit(1);
    }

    let config_dir_path = matches.value_of("config").map(ToString::to_string).unwrap_or_else(||
        shellexpand::tilde(default_config_dir_path).to_string());
    let config_dir_path = Path::new(&config_dir_path);
    let config_path = config_dir_path.join("config.yaml");

    let mut config = match load_config(config_path.to_str().unwrap()) {
        Ok(config) => config,
        Err(err) => {
            error!("Error while reading {:?} configuration file: {}.", config_path, err);
            process::exit(1);
        }
    };
    config.db_path = config_dir_path.join("db.sqlite").to_str().unwrap().to_owned();

    let action = match parse_arguments(&mut config, &matches) {
        Ok(action) => action,
        Err(err) => {
            error!("{}.", err);
            process::exit(1);
        },
    };

    debug!("{:#?}", config);
    (action, config)
}

fn parse_arguments(config: &mut Config, matches: &ArgMatches) -> GenericResult<Action> {
    if let Some(expire_time) = matches.value_of("cache_expire_time") {
        config.cache_expire_time = util::parse_duration(expire_time).map_err(|_| format!(
            "Invalid cache expire time: {:?}", expire_time))?;
    };

    let (command, matches) = matches.subcommand();
    let matches = matches.unwrap();

    let portfolio_name = portfolio::get(matches);

    Ok(match command {
        "analyse" => Action::Analyse(portfolio_name),

        "sync" => Action::Sync(portfolio_name),
        "buy" | "sell" | "cash" => {
            let cash_assets = Decimal::from_str(&cash_assets::get(matches))
                .map_err(|_| "Invalid cash assets value")?;

            if command == "cash" {
                Action::SetCashAssets(portfolio_name, cash_assets)
            } else {
                let shares = shares::get(matches).parse().map_err(|_| "Invalid shares number")?;
                let symbol = symbol::get(matches);

                match command {
                    "buy" => Action::Buy(portfolio_name, shares, symbol, cash_assets),
                    "sell" => Action::Sell(portfolio_name, shares, symbol, cash_assets),
                    _ => unreachable!(),
                }
            }
        },

        "show" => Action::Show {
            name: portfolio_name,
            flat: matches.is_present("flat"),
        },
        "rebalance" => Action::Rebalance {
            name: portfolio_name,
            flat: matches.is_present("flat"),
        },

        "tax-statement" => {
            let year = matches.value_of("YEAR").unwrap();
            let year = year.trim().parse::<i32>().ok()
                .and_then(|year| Date::from_ymd_opt(year, 1, 1).and(Some(year)))
                .ok_or_else(|| format!("Invalid year: {}", year))?;

            let tax_statement_path = matches.value_of("TAX_STATEMENT").map(|path| path.to_owned());

            Action::TaxStatement {
                name: portfolio_name,
                year: year,
                tax_statement_path: tax_statement_path,
            }
        },
        _ => unreachable!(),
    })
}

macro_rules! arg {
    ($id:ident, $name:expr, $help:expr) => {
        mod $id {
            use super::*;

            pub fn arg() -> Arg<'static, 'static> {
                Arg::with_name($name)
                    .help($help)
                    .required(true)
            }

            pub fn get(matches: &ArgMatches) -> String {
                matches.value_of($name).unwrap().to_owned()
            }
        }
    }
}

arg!(portfolio, "PORTFOLIO", "Portfolio name");
arg!(shares, "SHARES", "Shares");
arg!(symbol, "SYMBOL", "Symbol");
arg!(cash_assets, "CASH_ASSETS", "Current cash assets");