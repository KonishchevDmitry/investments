use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Command;
use easy_logging::LoggingConfig;
use log::{warn, error};

use investments::analysis;
use investments::core::EmptyResult;
use investments::config::Config;

fn main() -> ExitCode {
    let matches = Command::new("backtest")
        .about("Portfolio backtesting tool")
        .help_expected(true)
        .disable_help_subcommand(true)
        .args(Config::args())
        .get_matches();

    let (log_level, config_dir) = match Config::parse_args(&matches) {
        Ok(args) => args,
        Err(err) => {
            let _ = writeln!(io::stderr(), "{err}.");
            return ExitCode::FAILURE;
        },
    };

    let logging = LoggingConfig::new(module_path!(), log_level)
        .level_for("investments", log_level);

    if let Err(err) = logging.build() {
        let _ = writeln!(io::stderr(), "Failed to initialize the logging: {err}.");
        return ExitCode::FAILURE;
    }

    if let Err(err) = run(&config_dir) {
        error!("{err}.");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

pub fn run(config_dir: &Path) -> EmptyResult {
    let config = Config::new(config_dir)?;

    warn!("Not implemented yet.");
    analysis::backtest(&config, "SBMX")?;

    Ok(())
}