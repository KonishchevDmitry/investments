use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;

use clap::{App, Arg, AppSettings, SubCommand};
use log;
use serde_yaml;
use shellexpand;

use core::GenericResult;
use easy_logging;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(skip)]
    pub db_path: String,
}

pub enum Action {
    Analyse(String),
}

pub fn load() -> (Action, Config) {
    let default_config_dir_path = "~/.investment-tools";

    let matches = App::new("Investment tools")
        .about("\nHelp you with managing your investments")
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
            .arg(Arg::with_name("BROKER_STATEMENT")
                .help("Path to Interactive Brokers statement *.csv file")
                .required(true)))
        .global_setting(AppSettings::DisableVersion)
        .setting(AppSettings::SubcommandRequired)
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

    let action = match matches.subcommand() {
        ("analyse", Some(matches)) => Action::Analyse(
            matches.value_of("BROKER_STATEMENT").unwrap().to_owned()),
        _ => unreachable!(),
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

fn load_config(path: &str) -> GenericResult<Config> {
    let mut data = Vec::new();
    File::open(path)?.read_to_end(&mut data)?;
    Ok(serde_yaml::from_slice(&data)?)
}