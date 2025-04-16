use std::path::PathBuf;
use std::str::FromStr;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use clap::builder::NonEmptyStringValueParser;
use clap_complete::{self, Shell};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use strum::{EnumMessage, IntoEnumIterator};

use investments::analysis::PerformanceAnalysisMethod;
use investments::config::Config;
use investments::core::GenericResult;
use investments::time;
use investments::types::{Date, Decimal};

use super::action::Action;
use super::positions::PositionsParser;

lazy_static! {
    static ref WRAP_REGEX: Regex = Regex::new(r"(\S) *\n *(\S)").unwrap();
}

macro_rules! long_about {
    ($text:expr) => {{
        let text = WRAP_REGEX.replace_all(indoc::indoc!($text), "$1 $2");
        textwrap::fill(text.trim_matches('\n'), 100)
    }}
}

pub struct Parser {
    matches: Option<ArgMatches>,
    completion: Option<Vec<u8>>,

    bought: PositionsParser,
    sold: PositionsParser,
    to_sell: PositionsParser,
}

pub struct GlobalOptions {
    pub log_level: log::Level,
    pub config_dir: PathBuf,
}

impl Parser {
    pub fn new() -> Parser {
        Parser {
            matches: None,
            completion: None,

            bought: PositionsParser::new("Bought shares", false, true),
            sold: PositionsParser::new("Sold shares", true, true),
            to_sell: PositionsParser::new("Positions to sell", true, false),
        }
    }

    pub fn parse_global(&mut self) -> GenericResult<GlobalOptions> {
        let binary_name = "investments";

        let mut app = Command::new(binary_name)
            .about("Helps you with managing your investments")
            .version(env!("CARGO_PKG_VERSION"))
            .help_expected(true)
            .disable_help_subcommand(true)
            .subcommand_required(true)
            .arg_required_else_help(true)
            .args(Config::args())
            .args([
                Arg::new("cache_expire_time").short('e').long("cache-expire-time")
                    .help("Quote cache expire time (in $number{m|h|d} format)")
                    .value_name("DURATION")
                    .value_parser(time::parse_duration),
            ])

            .subcommand(Command::new("analyse")
                .about("Analyze portfolio performance")
                .long_about(long_about!("
                    Calculates average rate of return from cash investments by comparing portfolio
                    performance to performance of a bank deposit with exactly the same investments
                    and monthly capitalization.
                "))
                .args([
                    Arg::new("method").short('m').long("method")
                        .help(
                            PerformanceAnalysisMethod::iter().map(|method| {
                                format!("{} - {}", Into::<&'static str>::into(method), method.get_message().unwrap())
                            }).join(", ")
                        )
                        .value_parser(PerformanceAnalysisMethod::from_str)
                        .default_value(Into::<&'static str>::into(PerformanceAnalysisMethod::Real)),

                    Arg::new("all").short('a').long("all")
                        .help("Don't hide closed positions")
                        .action(ArgAction::SetTrue),

                    Arg::new("PORTFOLIO")
                        .help("Portfolio name (omit to show an aggregated result for all portfolios)")
                        .value_parser(NonEmptyStringValueParser::new()),
                ]))

            .subcommand(Command::new("show")
                .about("Show portfolio asset allocation")
                .args([
                    Arg::new("flat").short('f').long("flat")
                        .help("Flat view")
                        .action(ArgAction::SetTrue),

                    portfolio::arg(),
                ]))

            .subcommand(Command::new("sync")
                .about("Sync portfolio with broker statement")
                .arg(portfolio::arg()))

            .subcommand(Command::new("buy")
                .about("Add the specified stock shares to the portfolio")
                .args([
                    portfolio::arg(),
                    self.bought.arg(),
                    cash_assets::arg(),
                ]))

            .subcommand(Command::new("sell")
                .about("Remove the specified stock shares from the portfolio")
                .args([
                    portfolio::arg(),
                    self.sold.arg(),
                    cash_assets::arg(),
                ]))

            .subcommand(Command::new("cash")
                .about("Set current cash assets")
                .args([
                    portfolio::arg(),
                    cash_assets::arg(),
                ]))

            .subcommand(Command::new("rebalance")
                .about("Rebalance the portfolio according to the asset allocation configuration")
                .args([
                    Arg::new("flat").short('f').long("flat")
                        .help("Flat view")
                        .action(ArgAction::SetTrue),

                    portfolio::arg(),
                ]))

            .subcommand(Command::new("simulate-sell")
                .about("Simulate stock selling (calculates revenue, profit and taxes)")
                .args([
                    Arg::new("base_currency").short('b').long("base-currency")
                        .help("Actual asset base currency to calculate the profit in")
                        .value_name("CURRENCY")
                        .value_parser(NonEmptyStringValueParser::new()),

                    portfolio::arg(),
                    self.to_sell.arg(),
                ]))

            .subcommand(Command::new("tax-statement")
                .about("Generate tax statement")
                .long_about(long_about!("
                    Reads broker statements and alters *.dcX file (created by Russian tax program
                    named Декларация) by adding all required information about income from stock
                    selling, paid dividends and idle cash interest.

                    If tax statement file is not specified only outputs the data which is going to
                    be declared.
                "))
                .args([
                    portfolio::arg(),

                    Arg::new("YEAR")
                        .help("Year to generate the statement for")
                        .value_parser(parse_year),

                    Arg::new("TAX_STATEMENT")
                        .help("Path to tax statement *.dcX file")
                        .value_parser(value_parser!(PathBuf))
                ]))

            .subcommand(Command::new("cash-flow")
                .about("Generate cash flow report")
                .long_about("Generates cash flow report for tax inspection notification")
                .args([
                    portfolio::arg(),

                    Arg::new("YEAR")
                        .help("Year to generate the report for")
                        .value_parser(parse_year),
                ]))

            .subcommand(Command::new("deposits")
                .about("List deposits")
                .args([
                    Arg::new("date").short('d').long("date")
                        .help("Date to show information for (in DD.MM.YYYY format)")
                        .value_name("DATE")
                        .value_parser(time::parse_user_date),

                    Arg::new("cron").long("cron")
                        .help("cron mode (use for notifications about expiring and closed deposits)")
                        .action(ArgAction::SetTrue),
                ]))

            .subcommand(Command::new("metrics")
                .about("Generate Prometheus metrics for Node Exporter Textfile Collector")
                .arg(Arg::new("PATH")
                    .help("Path to write the metrics to")
                    .value_parser(value_parser!(PathBuf))
                    .required(true)))

            .subcommand(Command::new("completion")
                .about("Generate shell completion rules")
                .args([
                    Arg::new("shell").short('s').long("shell")
                        .help("Shell to generate completion rules for")
                        .value_name("SHELL")
                        .value_parser(value_parser!(Shell))
                        .default_value("bash"),

                    Arg::new("PATH")
                        .help("Path to save the rules to")
                        .value_parser(value_parser!(PathBuf))
                        .required(true)
                ]));

        let matches = app.get_matches_mut();
        let (log_level, config_dir) = Config::parse_args(&matches)?;

        {
            let mut app = app;
            let (command, matches) = matches.subcommand().unwrap();

            if command == "completion" {
                let mut completion = Vec::new();
                let shell = matches.get_one::<Shell>("shell").cloned().unwrap();
                clap_complete::generate(shell, &mut app, binary_name, &mut completion);
                self.completion = Some(completion);
            }
        }

        self.matches = Some(matches);

        Ok(GlobalOptions {log_level, config_dir})
    }

    pub fn parse(mut self, config: &mut Config) -> GenericResult<(String, Action)> {
        let matches = self.matches.take().unwrap();

        if let Some(expire_time) = matches.get_one("cache_expire_time").cloned() {
            config.cache_expire_time = expire_time;
        };

        let (command, matches) = matches.subcommand().unwrap();
        let action = self.parse_command(command, matches)?;

        Ok((command.to_owned(), action))
    }

    fn parse_command(&self, command: &str, matches: &ArgMatches) -> GenericResult<Action> {
        Ok(match command {
            "analyse" => Action::Analyse {
                name: matches.get_one("PORTFOLIO").cloned(),
                method: matches.get_one("method").cloned().unwrap(),
                show_closed_positions: matches.get_flag("all"),
            },

            "sync" => Action::Sync(portfolio::get(matches)),
            "buy" | "sell" | "cash" => {
                let name = portfolio::get(matches);
                let cash_assets = Decimal::from_str(&cash_assets::get(matches))
                    .map_err(|_| "Invalid cash assets value")?;

                match command {
                    "buy" => Action::Buy {
                        name, cash_assets,
                        positions: self.bought.parse(matches)?.unwrap().into_iter().map(|(symbol, shares)| {
                            (symbol, shares.unwrap())
                        }).collect(),
                    },
                    "sell" => Action::Sell {
                        name, cash_assets,
                        positions: self.sold.parse(matches)?.unwrap(),
                    },
                    "cash" => Action::SetCashAssets(name, cash_assets),
                    _ => unreachable!(),
                }
            },

            "show" => Action::Show {
                name: portfolio::get(matches),
                flat: matches.get_flag("flat"),
            },

            "rebalance" => Action::Rebalance {
                name: portfolio::get(matches),
                flat: matches.get_flag("flat"),
            },

            "simulate-sell" => Action::SimulateSell {
                name: portfolio::get(matches),
                positions: self.to_sell.parse(matches)?,
                base_currency: matches.get_one("base_currency").cloned(),
            },

            "tax-statement" => {
                Action::TaxStatement {
                    name: portfolio::get(matches),
                    year: matches.get_one("YEAR").cloned(),
                    tax_statement_path: matches.get_one("TAX_STATEMENT").cloned(),
                }
            },

            "cash-flow" => {
                Action::CashFlow {
                    name: portfolio::get(matches),
                    year: matches.get_one("YEAR").cloned(),
                }
            },

            "deposits" => {
                Action::Deposits {
                    date: matches.get_one("date").cloned().unwrap_or_else(time::today),
                    cron_mode: matches.get_flag("cron"),
                }
            },

            "metrics" => {
                Action::Metrics(matches.get_one("PATH").cloned().unwrap())
            },

            "completion" => Action::ShellCompletion {
                path: matches.get_one("PATH").cloned().unwrap(),
                data: self.completion.as_ref().unwrap().clone(),
            },

            _ => unreachable!(),
        })
    }
}

fn parse_year(year: &str) -> GenericResult<i32> {
    Ok(year.parse::<i32>().ok()
        .and_then(|year| Date::from_ymd_opt(year, 1, 1).and(Some(year)))
        .ok_or_else(|| format!("Invalid year: {}", year))?)
}

macro_rules! arg {
    ($id:ident, $name:expr, $help:expr) => {
        mod $id {
            use super::*;

            pub fn arg() -> Arg {
                Arg::new($name).help($help)
                    .value_parser(NonEmptyStringValueParser::new())
                    .required(true)
            }

            pub fn get(matches: &ArgMatches) -> String {
                matches.get_one($name).cloned().unwrap()
            }
        }
    }
}

arg!(portfolio, "PORTFOLIO", "Portfolio name");
arg!(cash_assets, "CASH_ASSETS", "Current cash assets");