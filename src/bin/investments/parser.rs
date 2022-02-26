use std::str::FromStr;

use clap::{Arg, ArgMatches, ArgEnum};
use clap_complete::{self, Shell};
use const_format::formatcp;

use investments::cli;
use investments::config::Config;
use investments::core::GenericResult;
use investments::time;
use investments::types::{Date, Decimal};

use super::action::Action;
use super::positions::PositionsParser;

pub struct Parser {
    matches: Option<ArgMatches>,
    completion: Option<Vec<u8>>,

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
            matches: None,
            completion: None,

            bought: PositionsParser::new("Bought shares", false, true),
            sold: PositionsParser::new("Sold shares", true, true),
            to_sell: PositionsParser::new("Positions to sell", true, false),
        }
    }

    pub fn parse_global(&mut self) -> GenericResult<GlobalOptions> {
        let binary_name = "investments";
        const DEFAULT_CONFIG_DIR_PATH: &str = "~/.investments";

        // App has very inconvenient lifetime requirements which always tend to become 'static due
        // to current implementation.
        let unsafe_parser = unsafe {
            &*(self as *const Parser)
        };

        let mut app = cli::new_app(binary_name, "Helps you with managing your investments")
            .version(env!("CARGO_PKG_VERSION"))
            .subcommand_required(true)
            .arg_required_else_help(true)
            .args([
                cli::new_arg("config", formatcp!("Configuration directory path [default: {}]", DEFAULT_CONFIG_DIR_PATH))
                    .short('c').long("config")
                    .value_name("PATH"),

                cli::new_arg("cache_expire_time", "Quote cache expire time (in $number{m|h|d} format)")
                    .short('e').long("cache-expire-time")
                    .value_name("DURATION"),

                cli::new_arg("verbose", "Set verbosity level")
                    .short('v').long("verbose")
                    .multiple_occurrences(true)
                    .max_occurrences(2),
            ])

            .subcommand(cli::new_subcommand(
                "analyse", "Analyze portfolio performance")
                .long_about("\
                    Calculates average rate of return from cash investments by comparing portfolio \
                    performance to performance of a bank deposit with exactly the same investments \
                    and monthly capitalization.")
                .args([
                    cli::new_arg("all", "Don't hide closed positions")
                        .short('a').long("all"),

                    cli::new_arg(
                        "PORTFOLIO",
                        "Portfolio name (omit to show an aggregated result for all portfolios)"),
                ]))

            .subcommand(cli::new_subcommand(
                "show", "Show portfolio asset allocation")
                .args([
                    cli::new_arg("flat", "Flat view")
                        .short('f').long("flat"),

                    portfolio::arg(),
                ]))

            .subcommand(cli::new_subcommand(
                "sync", "Sync portfolio with broker statement")
                .arg(portfolio::arg()))

            .subcommand(cli::new_subcommand(
                "buy", "Add the specified stock shares to the portfolio")
                .args([
                    portfolio::arg(),
                    unsafe_parser.bought.arg(),
                    cash_assets::arg(),
                ]))

            .subcommand(cli::new_subcommand(
                "sell", "Remove the specified stock shares from the portfolio")
                .args([
                    portfolio::arg(),
                    unsafe_parser.sold.arg(),
                    cash_assets::arg(),
                ]))

            .subcommand(cli::new_subcommand(
                "cash", "Set current cash assets")
                .args([
                    portfolio::arg(),
                    cash_assets::arg(),
                ]))

            .subcommand(cli::new_subcommand(
                "rebalance", "Rebalance the portfolio according to the asset allocation configuration")
                .args([
                    cli::new_arg("flat", "Flat view")
                        .short('f').long("flat"),

                    portfolio::arg(),
                ]))

            .subcommand(cli::new_subcommand(
                "simulate-sell", "Simulate stock selling (calculates revenue, profit and taxes)")
                .args([
                    cli::new_arg("base_currency", "Actual asset base currency to calculate the profit in")
                        .short('b').long("base-currency")
                        .value_name("CURRENCY"),

                    portfolio::arg(),
                    unsafe_parser.to_sell.arg(),
                ]))

            .subcommand(cli::new_subcommand(
                "tax-statement", "Generate tax statement")
                .long_about("\
                    Reads broker statements and alters *.dcX file (created by Russian tax program \
                    named Декларация) by adding all required information about income from stock \
                    selling, paid dividends and idle cash interest.\n\
                    \n\
                    If tax statement file is not specified only outputs the data which is going to \
                    be declared.")
                .args([
                    portfolio::arg(),
                    cli::new_arg("YEAR", "Year to generate the statement for"),
                    cli::new_arg("TAX_STATEMENT", "Path to tax statement *.dcX file"),
                ]))

            .subcommand(cli::new_subcommand(
                "cash-flow", "Generate cash flow report")
                .long_about("Generates cash flow report for tax inspection notification")
                .args([
                    portfolio::arg(),
                    cli::new_arg("YEAR", "Year to generate the report for"),
                ]))

            .subcommand(cli::new_subcommand(
                "deposits", "List deposits")
                .args([
                    cli::new_arg("date", "Date to show information for (in DD.MM.YYYY format)")
                        .short('d').long("date")
                        .value_name("DATE"),

                    cli::new_arg("cron", "cron mode (use for notifications about expiring and closed deposits)")
                        .long("cron"),
                ]))

            .subcommand(cli::new_subcommand(
                "metrics", "Generate Prometheus metrics for Node Exporter Textfile Collector")
                .arg(cli::new_arg("PATH", "Path to write the metrics to").required(true)))

            .subcommand(cli::new_subcommand(
                "completion", "Generate shell completion rules")
                .args([
                    cli::new_arg("shell", "Shell to generate completion rules for")
                        .short('s').long("shell").value_name("SHELL")
                        .possible_values(Shell::possible_values())
                        .default_value(Shell::Bash.to_possible_value().unwrap().get_name()),

                    cli::new_arg("PATH", "Path to save the rules to").required(true)
                ]));

        let matches = app.get_matches_mut();

        let log_level = match matches.occurrences_of("verbose") {
            0 => log::Level::Info,
            1 => log::Level::Debug,
            2 => log::Level::Trace,
            _ => return Err!("Invalid verbosity level"),
        };

        let config_dir = matches.value_of("config").map(ToString::to_string).unwrap_or_else(||
            shellexpand::tilde(DEFAULT_CONFIG_DIR_PATH).to_string());

        {
            let mut app = app;
            let (command, matches) = matches.subcommand().unwrap();

            if command == "completion" {
                let mut completion = Vec::new();
                let shell = matches.value_of_t::<Shell>("shell")?;
                clap_complete::generate(shell, &mut app, binary_name, &mut completion);
                self.completion = Some(completion);
            }
        }

        self.matches = Some(matches);

        Ok(GlobalOptions {log_level, config_dir})
    }

    pub fn parse(mut self, config: &mut Config) -> GenericResult<(String, Action)> {
        let matches = self.matches.take().unwrap();

        if let Some(expire_time) = matches.value_of("cache_expire_time") {
            config.cache_expire_time = time::parse_duration(expire_time).map_err(|_| format!(
                "Invalid cache expire time: {:?}", expire_time))?;
        };

        let (command, matches) = matches.subcommand().unwrap();
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
                    Some(date) => time::parse_user_date(date)?,
                    None => time::today(),
                };

                Action::Deposits {
                    date: date,
                    cron_mode: matches.is_present("cron"),
                }
            },

            "metrics" => {
                let path = matches.value_of("PATH").unwrap().to_owned();
                Action::Metrics(path)
            },

            "completion" => Action::ShellCompletion {
                path: matches.value_of("PATH").unwrap().into(),
                data: self.completion.as_ref().unwrap().clone(),
            },

            _ => unreachable!(),
        })
    }
}

fn get_year(matches: &ArgMatches) -> GenericResult<Option<i32>> {
    matches.value_of("YEAR").map(|year| {
        Ok(year.parse::<i32>().ok()
            .and_then(|year| Date::from_ymd_opt(year, 1, 1).and(Some(year)))
            .ok_or_else(|| format!("Invalid year: {}", year))?)
    }).transpose()
}

macro_rules! arg {
    ($id:ident, $name:expr, $help:expr) => {
        mod $id {
            use super::*;

            pub fn arg() -> Arg<'static> {
                cli::new_arg($name, $help).required(true)
            }

            pub fn get(matches: &ArgMatches) -> String {
                matches.value_of($name).unwrap().to_owned()
            }
        }
    }
}

arg!(portfolio, "PORTFOLIO", "Portfolio name");
arg!(cash_assets, "CASH_ASSETS", "Current cash assets");