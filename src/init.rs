use std::io::{self, Write};
use std::path::Path;
use std::process;

use clap::{App, Arg, AppSettings, SubCommand, ArgMatches};
use easy_logging;
use log;
use shellexpand;

use config::{Config, load_config};
use core::GenericResult;
use types::Date;

pub enum Action {
    Analyse(String),
    TaxStatement {
        year: i32,
        broker_statement_path: String,
        tax_statement_path: Option<String>,
    },
}

pub fn initialize() -> (Action, Config) {
    let default_config_dir_path = "~/.investments";

    let matches = App::new("Investments")
        .about("\nHelps you with managing your investments")
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("PATH")
            .help(&format!("Configuration directory path [default: {}]", default_config_dir_path))
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
            .arg(Arg::with_name("BROKER_STATEMENT")
                .help("Path to Interactive Brokers statement *.csv file")
                .required(true)))
        .subcommand(SubCommand::with_name("tax-statement")
            .about("Generate tax statement")
            .long_about(concat!(
                "\nReads Interactive Brokers statement and alters *.dcX file (Russian tax program ",
                "named Декларация) by adding all required information about income from paid ",
                "dividends.\n",
                "\nIf tax statement file is not specified only outputs the data which is going to ",
                "be declared."))
            .arg(Arg::with_name("YEAR")
                .help("Year to generate the statement for")
                .required(true))
            .arg(Arg::with_name("BROKER_STATEMENT")
                .help("Path to Interactive Brokers statement *.csv file")
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

    let action = match parse_arguments(&matches) {
        Ok(action) => action,
        Err(err) => {
            error!("{}.", err);
            process::exit(1);
        },
    };

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

    (action, config)
}

fn parse_arguments(matches: &ArgMatches) -> GenericResult<Action> {
    Ok(match matches.subcommand() {
        ("analyse", Some(matches)) => Action::Analyse(
            matches.value_of("BROKER_STATEMENT").unwrap().to_owned()),
        ("tax-statement", Some(matches)) => {
            let year = matches.value_of("YEAR").unwrap();
            let year = year.trim().parse::<i32>().ok()
                .and_then(|year| Date::from_ymd_opt(year, 1, 1).and(Some(year)))
                .ok_or_else(|| format!("Invalid year: {}", year))?;

            let broker_statement_path = matches.value_of("BROKER_STATEMENT").unwrap().to_owned();
            let tax_statement_path = matches.value_of("TAX_STATEMENT").map(|path| path.to_owned());

            Action::TaxStatement {
                year: year,
                broker_statement_path: broker_statement_path,
                tax_statement_path: tax_statement_path,
            }
        },
        _ => unreachable!(),
    })
}