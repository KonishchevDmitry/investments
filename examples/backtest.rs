use std::io::{self, Write};
use std::process::ExitCode;

use clap::Command;
use easy_logging::LoggingConfig;
use log::{warn, error};

use investments::analysis;
use investments::core::EmptyResult;
use investments::config::{CliConfig, Config};

fn main() -> ExitCode {
    let matches = Command::new("backtest")
        .about("Portfolio backtesting tool")
        .help_expected(true)
        .disable_help_subcommand(true)
        .args(Config::args())
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

    if let Err(err) = run(cli_config) {
        error!("{err}.");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

pub fn run(cli_config: CliConfig) -> EmptyResult {
    let config = Config::new(&cli_config.config_dir, cli_config.cache_expire_time)?;

    warn!("This is a work in progress tool.");
    analysis::backtest(&config)?;

    Ok(())
}