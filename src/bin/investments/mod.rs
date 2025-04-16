mod action;
mod parser;
mod positions;

#[macro_use] extern crate maplit;

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use log::error;

use investments::analysis;
use investments::cash_flow;
use investments::config::Config;
use investments::core::{EmptyResult, GenericResult};
use investments::db;
use investments::deposits;
use investments::metrics;
use investments::portfolio;
use investments::tax_statement;
use investments::telemetry::{Telemetry, TelemetryRecordBuilder};

use self::action::Action;
use self::parser::{Parser, GlobalOptions};

fn main() -> ExitCode {
    let mut parser = Parser::new();

    let global = match parser.parse_global() {
        Ok(global) => global,
        Err(err) => {
            let _ = writeln!(io::stderr(), "{err}.");
            return ExitCode::FAILURE;
        },
    };

    if let Err(err) = easy_logging::init(module_path!(), global.log_level) {
        let _ = writeln!(io::stderr(), "Failed to initialize the logging: {err}.");
        return ExitCode::FAILURE;
    }

    if let Err(err) = run(global, parser) {
        let message = err.to_string();

        if message.contains('\n') {
            error!("{err}");
        } else {
            error!("{err}.");
        }

        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn run(global: GlobalOptions, parser: Parser) -> EmptyResult {
    let mut config = Config::new(&global.config_dir)?;
    let (command, action) = parser.parse(&mut config)?;

    let telemetry = (!config.telemetry.disable).then(|| -> GenericResult<Telemetry> {
        let connection = db::connect(&config.db_path)?;
        let user_id = config.telemetry.user_id.map(|user_id| user_id.to_string());
        Telemetry::new(connection, user_id, "https://investments.konishchev.ru", btreemap! {
            // Dummy HTTPS request averages to Moscow:
            // * Paris    - 243 ms
            // * London   - 257 ms
            // * New York - 553 ms
             5 => Duration::from_millis(500),
            20 => Duration::from_millis(750),
        }, 100)
    }).transpose()?;

    let record: TelemetryRecordBuilder = match action {
        Action::Analyse {name, method, show_closed_positions} => {
            let (statistics, _, telemetry) = analysis::analyse(
                &config, name.as_deref(), show_closed_positions, &Default::default(), None, true)?;
            statistics.print(method);
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
        telemetry.add(record.build(&command))?;
    }

    Ok(())
}

fn write_shell_completion(path: &Path, data: &[u8]) -> EmptyResult {
    Ok(File::create(path)?.write_all(data)?)
}