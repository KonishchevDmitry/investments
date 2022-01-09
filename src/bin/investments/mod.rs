mod action;
mod parser;
mod positions;

#[macro_use] extern crate investments;
#[macro_use] extern crate maplit;

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::process;
use std::time::Duration;

use log::error;

use investments::analysis;
use investments::cash_flow;
use investments::config::Config;
use investments::core::EmptyResult;
use investments::db;
use investments::deposits;
use investments::metrics;
use investments::portfolio;
use investments::tax_statement;
use investments::telemetry::{Telemetry, TelemetryRecordBuilder};

use self::action::Action;
use self::parser::{Parser, GlobalOptions};

fn main() {
    let mut parser = Parser::new();

    let global = parser.parse_global().unwrap_or_else(|e| {
        let _ = writeln!(io::stderr(), "{}.", e);
        process::exit(1);
    });

    if let Err(e) = easy_logging::init(module_path!().split("::").next().unwrap(), global.log_level) {
        let _ = writeln!(io::stderr(), "Failed to initialize the logging: {}.", e);
        process::exit(1);
    }

    if let Err(e) = main_inner(global, parser) {
        error!("{}.", e);
        process::exit(1);
    }
}

fn main_inner(global: GlobalOptions, parser: Parser) -> EmptyResult {
    let config_dir_path = Path::new(&global.config_dir);
    let config_path = config_dir_path.join("config.yaml");

    let mut config = Config::load(config_path.to_str().unwrap()).map_err(|e| format!(
        "Error while reading {:?} configuration file: {}", config_path, e))?;
    config.db_path = config_dir_path.join("db.sqlite").to_str().unwrap().to_owned();

    let (command, action) = parser.parse(&mut config)?;
    run(config, &command, action)
}

fn run(config: Config, command: &str, action: Action) -> EmptyResult {
    let telemetry = if config.telemetry.disable {
        None
    } else {
        let connection = db::connect(&config.db_path)?;
        Some(Telemetry::new(connection, btreemap! {
            // Dummy HTTPS request averages to Moscow:
            // * Paris    - 243 ms
            // * London   - 257 ms
            // * New York - 553 ms
             5 => Duration::from_millis(500),
            20 => Duration::from_millis(750),
        }, 100)?)
    };

    let record: TelemetryRecordBuilder = match action {
        Action::Analyse {name, show_closed_positions} => {
            let (statistics, _, telemetry) = analysis::analyse(
                &config, name.as_deref(), show_closed_positions, None, true)?;
            statistics.print();
            telemetry
        },
        Action::SimulateSell {name, positions, base_currency} => analysis::simulate_sell(
            &config, &name, positions, base_currency.as_deref())?,

        Action::Sync(name) => portfolio::sync(&config, &name)?,
        Action::Buy {name, positions, cash_assets} =>
            portfolio::buy(&config, &name, &positions, cash_assets)?,
        Action::Sell {name, positions, cash_assets} =>
            portfolio::sell(&config, &name, &positions, cash_assets)?,
        Action::SetCashAssets(name, cash_assets) =>
            portfolio::set_cash_assets(&config, &name, cash_assets)?,

        Action::Show {name, flat} => portfolio::show(&config, &name, flat)?,
        Action::Rebalance {name, flat} => portfolio::rebalance(&config, &name, flat)?,

        Action::TaxStatement {name, year, tax_statement_path} =>
            tax_statement::generate_tax_statement(
                &config, &name, year, tax_statement_path.as_deref())?,
        Action::CashFlow {name, year} =>
            cash_flow::generate_cash_flow_report(&config, &name, year)?,

        Action::Deposits {date, cron_mode} => {
            deposits::list(
                &config.get_tax_country(), config.deposits, date, cron_mode,
                config.notify_deposit_closing_days);
            TelemetryRecordBuilder::new()
        },

        Action::Metrics(path) => metrics::collect(&config, &path)?,

        Action::ShellCompletion {path, data} => {
            write_shell_completion(&path, &data).map_err(|e| format!(
                "Failed to write {:?}: {}", path, e))?;
            TelemetryRecordBuilder::new()
        },
    };

    if let Some(telemetry) = telemetry.as_ref() {
        telemetry.add(record.build(command))?;
    }

    Ok(())
}

fn write_shell_completion(path: &Path, data: &[u8]) -> EmptyResult {
    Ok(File::create(path)?.write_all(data)?)
}