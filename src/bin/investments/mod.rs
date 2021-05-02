extern crate investments;

use std::process;

use log::error;

use investments::analysis;
use investments::cash_flow;
use investments::config::Config;
use investments::core::EmptyResult;
use investments::deposits;
use investments::metrics;
use investments::portfolio;
use investments::tax_statement;
use investments::telemetry::{TelemetryRecord, TelemetryRecordBuilder};

use self::init::{Action, initialize};

mod init;

// TODO: Features to implement:
// * Declare losses in tax statement: commissions and loss from previous years
// * XLS for tax inspector

fn main() {
    let (config, command, action) = initialize();

    if let Err(e) = run(config, &command, action) {
        error!("{}.", e);
        process::exit(1);
    }
}

fn run(config: Config, command: &str, action: Action) -> EmptyResult {
    let _: TelemetryRecord = match action {
        Action::Analyse {name, show_closed_positions} => {
            let (statistics, _, telemetry) = analysis::analyse(
                &config, name.as_deref(), show_closed_positions, None, true)?;
            statistics.print();
            telemetry
        },
        Action::SimulateSell {name, positions, base_currency} => analysis::simulate_sell(
            &config, &name, positions, base_currency.as_deref())?,

        Action::Sync(name) => portfolio::sync(&config, &name)?,
        Action::Buy {name, shares, symbol, cash_assets} =>
            portfolio::buy(&config, &name, shares, &symbol, cash_assets)?,
        Action::Sell {name, shares, symbol, cash_assets} =>
            portfolio::sell(&config, &name, shares, &symbol, cash_assets)?,
        Action::SetCashAssets(name, cash_assets) =>
            portfolio::set_cash_assets(&config, &name, cash_assets)?,

        Action::Show {name, flat} => portfolio::show(&config, &name, flat)?,
        Action::Rebalance {name, flat} => portfolio::rebalance(&config, &name, flat)?,

        Action::TaxStatement {name, year, tax_statement_path} =>
            tax_statement::generate_tax_statement(
                &config, &name, year, tax_statement_path.as_deref())?,
        Action::CashFlow {name, year} =>
            cash_flow::generate_cash_flow_report(&config, &name, year)?,

        Action::Deposits { date, cron_mode } => {
            deposits::list(
                &config.get_tax_country(), config.deposits, date, cron_mode,
                config.notify_deposit_closing_days);
            TelemetryRecordBuilder::new()
        },

        Action::Metrics(path) => metrics::collect(&config, &path)?,
    }.build(command);

    Ok(())
}
