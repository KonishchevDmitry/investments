extern crate investments;

use std::process;

use log::error;

use investments::analyse;
use investments::config::Config;
use investments::core::EmptyResult;
use investments::deposits;
use investments::portfolio;
use investments::tax_statement;

use self::init::{Action, initialize};

mod init;

// TODO: Features to implement:
// * Stock split support
// * Declare losses in tax statement: commissions and loss from previous years
// * Tax agent support
// * XLS for tax inspector
// * Free commissions (considering monthly minimum fee)
// * T+2 mode support for IB tax statement? (Trade Confirmations broker statement shows the info)

fn main() {
    let (action, config) = initialize();

    if let Err(e) = run(action, config) {
        error!("{}.", e);
        process::exit(1);
    }
}

fn run(action: Action, config: Config) -> EmptyResult {
    match action {
        Action::Analyse {name, show_closed_positions} => analyse::analyse(
            &config, &name, show_closed_positions)?,
        Action::SimulateSell {name, positions} => analyse::simulate_sell(
            &config, &name, &positions)?,

        Action::Sync(name) => portfolio::sync(&config, &name)?,
        Action::Buy(name, shares, symbol, cash_assets) =>
            portfolio::buy(&config, &name, shares, &symbol, cash_assets)?,
        Action::Sell(name, shares, symbol, cash_assets) =>
            portfolio::sell(&config, &name, shares, &symbol, cash_assets)?,
        Action::SetCashAssets(name, cash_assets) =>
            portfolio::set_cash_assets(&config, &name, cash_assets)?,

        Action::Show { name, flat } => portfolio::show(&config, &name, flat)?,
        Action::Rebalance { name, flat } => portfolio::rebalance(&config, &name, flat)?,

        Action::TaxStatement { name, year, tax_statement_path } =>
            tax_statement::generate_tax_statement(
                &config, &name, year, tax_statement_path.as_ref().map(String::as_str))?,

        Action::Deposits { date, cron_mode } => deposits::list(
            config.deposits, date, cron_mode, config.notify_deposit_closing_days),
    };

    Ok(())
}
