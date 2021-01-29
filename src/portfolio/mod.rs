use std::collections::hash_map::Entry;
use std::rc::Rc;

use crate::broker_statement::BrokerStatement;
use crate::config::{Config, PortfolioConfig};
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::quotes::Quotes;
use crate::types::Decimal;

use self::asset_allocation::Portfolio;
use self::assets::Assets;
use self::formatting::print_portfolio;

mod asset_allocation;
mod assets;
mod formatting;
mod rebalancing;

pub fn sync(config: &Config, portfolio_name: &str) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;
    let database = db::connect(&config.db_path)?;

    let statement = BrokerStatement::read(
        broker, &portfolio.statements, &portfolio.symbol_remapping, &portfolio.instrument_names,
        portfolio.get_tax_remapping()?, false)?;
    statement.check_date();

    let assets = Assets::new(statement.cash_assets, statement.open_positions);
    assets.validate(&portfolio)?;
    assets.save(database, &portfolio.name)?;

    Ok(())
}

pub fn buy(config: &Config, portfolio_name: &str, shares: Decimal, symbol: &str, cash_assets: Decimal) -> EmptyResult {
    modify_assets(config, portfolio_name, |portfolio, assets| {
        if portfolio.get_stock_symbols().get(symbol).is_none() {
            return Err!("Unable to buy {}: it's not specified in asset allocation configuration",
                symbol);
        }

        assets.stocks.entry(symbol.to_owned())
            .and_modify(|current_shares| *current_shares = (*current_shares + shares).normalize())
            .or_insert(shares);

        set_cash_assets_impl(portfolio, assets, cash_assets)
    })
}

pub fn sell(config: &Config, portfolio_name: &str, shares: Decimal, symbol: &str, cash_assets: Decimal) -> EmptyResult {
    modify_assets(config, portfolio_name, |portfolio, assets| {
        let mut entry = match assets.stocks.entry(symbol.to_owned()) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => return Err!("The portfolio has no open {} positions", symbol),
        };

        let current_shares = entry.get_mut();
        if shares > *current_shares {
            return Err!("Unable to sell {} shares of {}: the portfolio contains only {} shares",
                shares, symbol, current_shares);
        }

        if shares == *current_shares {
            entry.remove();
        } else {
            *current_shares = (*current_shares - shares).normalize();
        }

        set_cash_assets_impl(portfolio, assets, cash_assets)
    })
}

pub fn set_cash_assets(config: &Config, portfolio_name: &str, cash_assets: Decimal) -> EmptyResult {
    modify_assets(config, portfolio_name, |portfolio, assets| {
        set_cash_assets_impl(portfolio, assets, cash_assets)
    })
}

fn modify_assets<F>(config: &Config, portfolio_name: &str, modify: F) -> EmptyResult
    where F: Fn(&PortfolioConfig, &mut Assets) -> EmptyResult
{
    let portfolio = config.get_portfolio(portfolio_name)?;
    let database = db::connect(&config.db_path)?;

    let mut assets = Assets::load(database.clone(), &portfolio.name)?;
    modify(portfolio, &mut assets)?;
    assets.save(database, &portfolio.name)?;

    Ok(())
}

fn set_cash_assets_impl(portfolio: &PortfolioConfig, assets: &mut Assets, cash_assets: Decimal) -> EmptyResult {
    assets.cash.clear();
    assets.cash.deposit(Cash::new(portfolio.currency()?, cash_assets));
    Ok(())
}

pub fn show(config: &Config, portfolio_name: &str, flat: bool) -> EmptyResult {
    process(config, portfolio_name, false, flat)
}

pub fn rebalance(config: &Config, portfolio_name: &str, flat: bool) -> EmptyResult {
    process(config, portfolio_name, true, flat)
}

fn process(config: &Config, portfolio_name: &str, rebalance: bool, flat: bool) -> EmptyResult {
    let portfolio_config = config.get_portfolio(portfolio_name)?;
    let database = db::connect(&config.db_path)?;

    let quotes = Rc::new(Quotes::new(&config, database.clone())?);
    let converter = CurrencyConverter::new(database.clone(), Some(quotes.clone()), false);

    let assets = Assets::load(database, &portfolio_config.name)?;
    assets.validate(&portfolio_config)?;

    let mut portfolio = Portfolio::load(config, portfolio_config, assets, &converter, &quotes)?;
    if rebalance {
        rebalancing::rebalance_portfolio(&mut portfolio, converter)?;
    }

    print_portfolio(portfolio, flat);

    Ok(())
}