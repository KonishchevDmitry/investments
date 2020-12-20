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

use self::init::{Action, initialize};

mod init;

// TODO: Features to implement:
// * Declare losses in tax statement: commissions and loss from previous years
// * Tax agent support
// * XLS for tax inspector
// * Free commissions (considering monthly minimum fee)
// * Tiered commissions support for BCS broker

fn main() {
    let (action, config) = initialize();

    if let Err(e) = run(action, config) {
        error!("{}.", e);
        process::exit(1);
    }
}

fn run(action: Action, config: Config) -> EmptyResult {
    match action {
        Action::Analyse {name, show_closed_positions} => {
            let (statistics, _) = analysis::analyse(
                &config, name.as_deref(), show_closed_positions, None, true)?;
            statistics.print();
        },
        Action::SimulateSell {name, positions, base_currency} => analysis::simulate_sell(
            &config, &name, &positions, base_currency.as_deref())?,

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

        Action::Deposits { date, cron_mode } => deposits::list(
            config.deposits, date, cron_mode, config.notify_deposit_closing_days),

        Action::Metrics(path) => metrics::collect(&config, &path)?,
    };

    Ok(())
}
