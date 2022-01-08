use std::str::FromStr;

use clap::{App, Arg, ArgMatches, AppSettings};

use investments::config::Config;
use investments::core::GenericResult;
use investments::time;
use investments::types::{Date, Decimal};

use super::action::Action;
use super::positions::PositionsParser;

pub struct Parser {
    matches: ArgMatches,

    bought: PositionsParser,
    sold: PositionsParser,
    to_sell: PositionsParser,
}

pub struct GlobalOptions {
    pub log_level: log::Level,
    pub config_dir: String,
}

impl Parser {
    pub fn new() -> Parser {
        Parser {
            // FIXME(konishchev): HERE
            matches: ArgMatches::default(),

            // FIXME(konishchev): HERE
            bought: PositionsParser::new("Bought shares", false, true),
            sold: PositionsParser::new("Sold shares", true, true),
            to_sell: PositionsParser::new("Positions to sell", true, false),
        }
    }

    pub fn parse_global(&mut self) -> GenericResult<GlobalOptions> {
        // FIXME(konishchev): HERE
        // ArgMatches has very inconvenient lifetime requirements for some reason
        let unsafe_parser = unsafe {
            &*(self as *const Parser)
        };

        // FIXME(konishchev): HERE
        let default_config_dir_path = "~/.investments";
        let config_help = format!("Configuration directory path [default: {}]", default_config_dir_path);
        let config_help_str = unsafe {
            &*(config_help.as_str() as *const str)
        };

        self.matches = App::new("Investments")
            .about("\nHelps you with managing your investments")
            // FIXME(konishchev): Support
            // FIXME(konishchev): help: set vs sets
            .version(if cfg!(debug_assertions) {
                "devel"
            } else {
                env!("CARGO_PKG_VERSION")
            })

            .arg(Arg::new("config")
                .short('c')
                .long("config")
                .value_name("PATH")
                .takes_value(true)
                .help(config_help_str))

            .arg(Arg::new("cache_expire_time")
                .short('e')
                .long("cache-expire-time")
                .value_name("DURATION")
                .takes_value(true)
                .help("Quote cache expire time (in $number{m|h|d} format)"))

            .arg(Arg::new("verbose")
                .short('v')
                .long("verbose")
                // FIXME(konishchev): HERE
                .multiple_occurrences(true)
                .help("Sets the level of verbosity"))

            .subcommand(App::new("analyse")
                .about("Analyze portfolio performance")
                .long_about(concat!(
                "\nCalculates average rate of return from cash investments by comparing portfolio ",
                "performance to performance of a bank deposit with exactly the same investments ",
                "and monthly capitalization."))
                .arg(Arg::new("all")
                    .short('a')
                    .long("all")
                    .help("Don't hide closed positions"))
                .arg(Arg::new("PORTFOLIO")
                    .help("Portfolio name (omit to show an aggregated result for all portfolios)")))

            .subcommand(App::new("show")
                .about("Show portfolio's asset allocation")
                .arg(Arg::new("flat")
                    .short('f')
                    .long("flat")
                    .help("Flat view"))
                .arg(portfolio::arg()))

            .subcommand(App::new("sync")
                .about("Sync portfolio with broker statement")
                .arg(portfolio::arg()))

            .subcommand(App::new("buy")
                .about("Add the specified stock shares to the portfolio")
                .arg(portfolio::arg())
                .arg(unsafe_parser.bought.arg())
                .arg(cash_assets::arg()))

            .subcommand(App::new("sell")
                .about("Remove the specified stock shares from the portfolio")
                .arg(portfolio::arg())
                .arg(unsafe_parser.sold.arg())
                .arg(cash_assets::arg()))

            .subcommand(App::new("cash")
                .about("Set current cash assets")
                .arg(portfolio::arg())
                .arg(cash_assets::arg()))

            .subcommand(App::new("rebalance")
                .about("Rebalance the portfolio according to the asset allocation configuration")
                .arg(Arg::new("flat")
                    .short('f')
                    .long("flat")
                    .help("Flat view"))
                .arg(portfolio::arg()))

            .subcommand(App::new("simulate-sell")
                .about("Simulates stock selling (calculates revenue, profit and taxes)")
                .arg(Arg::new("base_currency")
                    .short('b')
                    .long("base-currency")
                    .value_name("CURRENCY")
                    .takes_value(true)
                    .help("Actual asset base currency to calculate the profit in"))
                .arg(portfolio::arg())
                .arg(unsafe_parser.to_sell.arg()))

            .subcommand(App::new("tax-statement")
                .about("Generate tax statement")
                .long_about(concat!(
                "\nReads broker statements and alters *.dcX file (created by Russian tax program ",
                "named Декларация) by adding all required information about income from stock ",
                "selling, paid dividends and idle cash interest.\n",
                "\nIf tax statement file is not specified only outputs the data which is going to ",
                "be declared."))
                .arg(portfolio::arg())
                .arg(Arg::new("YEAR")
                    .help("Year to generate the statement for"))
                .arg(Arg::new("TAX_STATEMENT")
                    .help("Path to tax statement *.dcX file")))

            .subcommand(App::new("cash-flow")
                .about("Generate cash flow report")
                .long_about("Generates cash flow report for tax inspection notification")
                .arg(portfolio::arg())
                .arg(Arg::new("YEAR")
                    .help("Year to generate the report for")))

            .subcommand(App::new("deposits")
                .about("List deposits")
                .arg(Arg::new("date")
                    .short('d')
                    .long("date")
                    .value_name("DATE")
                    .help("Date to show information for (in DD.MM.YYYY format)")
                    .takes_value(true))
                .arg(Arg::new("cron")
                    .long("cron")
                    .help("cron mode (use for notifications about expiring and closed deposits)")))

            .subcommand(App::new("metrics")
                .about("Generate Prometheus metrics for Node Exporter Textfile Collector")
                .arg(Arg::new("PATH")
                    .help("Path to write the metrics to")
                    .required(true)))

            .global_setting(AppSettings::DisableVersionFlag)
            .unset_setting(AppSettings::DisableVersionFlag)

            .global_setting(AppSettings::DisableHelpSubcommand)
            .global_setting(AppSettings::DeriveDisplayOrder)
            .setting(AppSettings::SubcommandRequiredElseHelp)
            .get_matches();

        let log_level = match self.matches.occurrences_of("verbose") {
            0 => log::Level::Info,
            1 => log::Level::Debug,
            2 => log::Level::Trace,
            _ => return Err!("Invalid verbosity level"),
        };

        let config_dir = self.matches.value_of("config").map(ToString::to_string).unwrap_or_else(||
            shellexpand::tilde(default_config_dir_path).to_string());

        Ok(GlobalOptions {log_level, config_dir})
    }

    pub fn parse(self, config: &mut Config) -> GenericResult<(String, Action)> {
        if let Some(expire_time) = self.matches.value_of("cache_expire_time") {
            config.cache_expire_time = time::parse_duration(expire_time).map_err(|_| format!(
                "Invalid cache expire time: {:?}", expire_time))?;
        };

        let (command, matches) = self.matches.subcommand().unwrap();
        let action = self.parse_command(command, matches)?;

        Ok((command.to_owned(), action))
    }

    fn parse_command(&self, command: &str, matches: &ArgMatches) -> GenericResult<Action> {
        Ok(match command {
            "analyse" => Action::Analyse {
                name: matches.value_of("PORTFOLIO").map(ToOwned::to_owned),
                show_closed_positions: matches.is_present("all"),
            },

            "sync" => Action::Sync(portfolio::get(matches)),
            "buy" | "sell" | "cash" => {
                let name = portfolio::get(matches);
                let cash_assets = Decimal::from_str(&cash_assets::get(matches))
                    .map_err(|_| "Invalid cash assets value")?;

                match command {
                    "buy" => Action::Buy {
                        name, cash_assets,
                        positions: self.bought.parse(matches)?.into_iter().map(|(symbol, shares)| {
                            (symbol, shares.unwrap())
                        }).collect(),
                    },
                    "sell" => Action::Sell {
                        name, cash_assets,
                        positions: self.sold.parse(matches)?,
                    },
                    "cash" => Action::SetCashAssets(name, cash_assets),
                    _ => unreachable!(),
                }
            },

            "show" => Action::Show {
                name: portfolio::get(matches),
                flat: matches.is_present("flat"),
            },

            "rebalance" => Action::Rebalance {
                name: portfolio::get(matches),
                flat: matches.is_present("flat"),
            },

            "simulate-sell" => Action::SimulateSell {
                name: portfolio::get(matches),
                positions: self.to_sell.parse(matches)?,
                base_currency: matches.value_of("base_currency").map(ToOwned::to_owned),
            },

            "tax-statement" => {
                let tax_statement_path = matches.value_of("TAX_STATEMENT").map(|path| path.to_owned());

                Action::TaxStatement {
                    name: portfolio::get(matches),
                    year: get_year(matches)?,
                    tax_statement_path: tax_statement_path,
                }
            },

            "cash-flow" => {
                Action::CashFlow {
                    name: portfolio::get(matches),
                    year: get_year(matches)?,
                }
            },

            "deposits" => {
                let date = match matches.value_of("date") {
                    Some(date) => time::parse_date(date, "%d.%m.%Y")?,
                    None => time::today(),
                };

                return Ok(Action::Deposits {
                    date: date,
                    cron_mode: matches.is_present("cron"),
                });
            },

            "metrics" => {
                let path = matches.value_of("PATH").unwrap().to_owned();
                return Ok(Action::Metrics(path))
            },

            _ => unreachable!(),
        })
    }
}

fn get_year(matches: &ArgMatches) -> GenericResult<Option<i32>> {
    Ok(match matches.value_of("YEAR") {
        Some(year) => {
            Some(year.trim().parse::<i32>().ok()
                .and_then(|year| Date::from_ymd_opt(year, 1, 1).and(Some(year)))
                .ok_or_else(|| format!("Invalid year: {}", year))?)
        },
        None => None,
    })
}

macro_rules! arg {
    ($id:ident, $name:expr, $help:expr) => {
        mod $id {
            use super::*;

            pub fn arg() -> Arg<'static> {
                Arg::new($name)
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
arg!(cash_assets, "CASH_ASSETS", "Current cash assets");