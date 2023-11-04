use std::collections::hash_map::Entry;
use std::rc::Rc;

use crate::broker_statement::{BrokerStatement, ReadingStrictness};
use crate::config::{Config, PortfolioConfig};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::quotes::Quotes;
use crate::telemetry::TelemetryRecordBuilder;
use crate::types::Decimal;

use self::asset_allocation::Portfolio;
use self::assets::Assets;
use self::formatting::print_portfolio;

mod asset_allocation;
mod assets;
mod formatting;
mod rebalancing;

pub fn sync(config: &Config, portfolio_name: &str) -> GenericResult<TelemetryRecordBuilder> {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;
    let database = db::connect(&config.db_path)?;

    let statement = BrokerStatement::read(
        broker, portfolio.statements_path()?, &portfolio.symbol_remapping, &portfolio.instrument_internal_ids,
        &portfolio.instrument_names, portfolio.get_tax_remapping()?, &portfolio.tax_exemptions,
        &portfolio.corporate_actions, ReadingStrictness::empty())?;
    statement.check_date();

    let assets = Assets::new(statement.assets.cash, statement.open_positions);
    assets.validate(portfolio)?;
    assets.save(database, &portfolio.name)?;

    Ok(TelemetryRecordBuilder::new_with_broker(portfolio.broker))
}

pub fn buy(
    config: &Config, portfolio_name: &str, positions: &[(String, Decimal)], cash_assets: Decimal,
) -> GenericResult<TelemetryRecordBuilder> {
    modify_assets(config, portfolio_name, |portfolio, assets| {
        let asset_allocation_symbols = portfolio.get_stock_symbols();

        for (symbol, quantity) in positions {
            if asset_allocation_symbols.get(symbol).is_none() {
                return Err!(
                    "Unable to buy {}: it's not specified in asset allocation configuration",
                    symbol);
            }

            assets.stocks.entry(symbol.to_owned())
                .and_modify(|current| *current = (*current + quantity).normalize())
                .or_insert(*quantity);
        }

        set_cash_assets_impl(portfolio, assets, cash_assets)
    })
}

pub fn sell(
    config: &Config, portfolio_name: &str, positions: &[(String, Option<Decimal>)],
    cash_assets: Decimal,
) -> GenericResult<TelemetryRecordBuilder> {
    modify_assets(config, portfolio_name, |portfolio, assets| {
        for (symbol, quantity) in positions {
            let mut entry = match assets.stocks.entry(symbol.to_owned()) {
                Entry::Occupied(entry) => entry,
                Entry::Vacant(_) => return Err!("The portfolio has no open {} positions", symbol),
            };

            let current = entry.get_mut();
            let quantity = match *quantity {
                Some(quantity) => quantity,
                None => *current,
            };

            if quantity > *current {
                return Err!(
                    "Unable to sell {} shares of {}: the portfolio contains only {} shares",
                    quantity, symbol, current);
            }

            if quantity == *current {
                entry.remove();
            } else {
                *current = (*current - quantity).normalize();
            }
        }

        set_cash_assets_impl(portfolio, assets, cash_assets)
    })
}

pub fn set_cash_assets(config: &Config, portfolio_name: &str, cash_assets: Decimal) -> GenericResult<TelemetryRecordBuilder> {
    modify_assets(config, portfolio_name, |portfolio, assets| {
        set_cash_assets_impl(portfolio, assets, cash_assets)
    })
}

fn modify_assets<F>(config: &Config, portfolio_name: &str, modify: F) -> GenericResult<TelemetryRecordBuilder>
    where F: Fn(&PortfolioConfig, &mut Assets) -> EmptyResult
{
    let portfolio = config.get_portfolio(portfolio_name)?;
    let database = db::connect(&config.db_path)?;

    let mut assets = Assets::load(database.clone(), &portfolio.name)?;
    modify(portfolio, &mut assets)?;
    assets.save(database, &portfolio.name)?;

    Ok(TelemetryRecordBuilder::new_with_broker(portfolio.broker))
}

fn set_cash_assets_impl(portfolio: &PortfolioConfig, assets: &mut Assets, cash_assets: Decimal) -> EmptyResult {
    assets.cash.clear();
    assets.cash.deposit(Cash::new(portfolio.currency()?, cash_assets));
    Ok(())
}

pub fn show(config: &Config, portfolio_name: &str, flat: bool) -> GenericResult<TelemetryRecordBuilder> {
    process(config, portfolio_name, false, flat)
}

pub fn rebalance(config: &Config, portfolio_name: &str, flat: bool) -> GenericResult<TelemetryRecordBuilder> {
    process(config, portfolio_name, true, flat)
}

fn process(config: &Config, portfolio_name: &str, rebalance: bool, flat: bool) -> GenericResult<TelemetryRecordBuilder> {
    let portfolio_config = config.get_portfolio(portfolio_name)?;
    let broker = portfolio_config.broker.get_info(config, portfolio_config.plan.as_ref())?;
    let database = db::connect(&config.db_path)?;

    let quotes = Rc::new(Quotes::new(config, database.clone())?);
    let converter = CurrencyConverter::new(database.clone(), Some(quotes.clone()), false);

    let assets = Assets::load(database, &portfolio_config.name)?;
    assets.validate(portfolio_config)?;

    let statement = portfolio_config.statements.as_ref().map(|path| {
        BrokerStatement::read(
            broker.clone(), path, &portfolio_config.symbol_remapping,
            &portfolio_config.instrument_internal_ids, &portfolio_config.instrument_names,
            portfolio_config.get_tax_remapping()?, &portfolio_config.tax_exemptions,
            &portfolio_config.corporate_actions, ReadingStrictness::empty())
    }).transpose()?;

    let mut portfolio = Portfolio::load(
        portfolio_config, broker, assets, statement.as_ref(), &converter, &quotes)?;

    if rebalance {
        rebalancing::rebalance_portfolio(&mut portfolio, converter)?;
    }

    print_portfolio(portfolio, flat);

    Ok(TelemetryRecordBuilder::new_with_broker(portfolio_config.broker))
}