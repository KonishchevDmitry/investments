use broker_statement::BrokerStatement;
use config::{Config, PortfolioConfig};
use core::EmptyResult;
use currency::Cash;
use currency::converter::CurrencyConverter;
use db;
use quotes::Quotes;
use types::Decimal;

use self::asset_allocation::Portfolio;
use self::assets::Assets;
use self::formatting::print_portfolio;

mod asset_allocation;
mod assets;
mod formatting;

pub fn sync(config: &Config, portfolio_name: &str) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let database = db::connect(&config.db_path)?;

    let statement = BrokerStatement::read(config, portfolio.broker, &portfolio.statements)?;
    statement.check_date();

    let assets = Assets::new(statement.cash_assets, statement.open_positions);
    assets.validate(&portfolio)?;
    assets.save(database, &portfolio.name)?;

    Ok(())
}

pub fn buy(config: &Config, portfolio_name: &str, shares: u32, symbol: &str, cash_assets: Decimal) -> EmptyResult {
    modify_assets(config, portfolio_name, |portfolio, assets| {
        if portfolio.get_stock_symbols().get(symbol).is_none() {
            return Err!("Unable to buy {}: it's not specified in asset allocation configuration",
                symbol);
        }

        let current_shares = assets.stocks.remove(symbol).unwrap_or(0);
        assets.stocks.insert(symbol.to_owned(), current_shares + shares);

        set_cash_assets_impl(portfolio, assets, cash_assets)
    })
}

pub fn sell(config: &Config, portfolio_name: &str, shares: u32, symbol: &str, cash_assets: Decimal) -> EmptyResult {
    modify_assets(config, portfolio_name, |portfolio, assets| {
        let current_shares = match assets.stocks.remove(symbol) {
            Some(current_shares) => current_shares,
            None => return Err!("The portfolio have no open {} positions", symbol),
        };

        if current_shares < shares {
            return Err!("Unable to sell {} shares of {}: the portfolio contains only {} shares",
                shares, symbol, current_shares);
        } else if current_shares > shares {
            assets.stocks.insert(symbol.to_owned(), current_shares - shares);
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
    assets.save(database.clone(), &portfolio.name)?;

    Ok(())
}

fn set_cash_assets_impl(portfolio: &PortfolioConfig, assets: &mut Assets, cash_assets: Decimal) -> EmptyResult {
    let currency = portfolio.currency.as_ref().ok_or_else(||
        "The portfolio's currency is not specified in the config")?;

    assets.cash.clear();
    assets.cash.deposit(Cash::new(&currency, cash_assets));

    Ok(())
}

pub fn show(config: &Config, portfolio_name: &str) -> EmptyResult {
    let portfolio_config = config.get_portfolio(portfolio_name)?;
    let database = db::connect(&config.db_path)?;

    let converter = CurrencyConverter::new(database.clone(), false);
    let mut quotes = Quotes::new(&config, database.clone());

    let assets = Assets::load(database, &portfolio_config.name)?;
    assets.validate(&portfolio_config)?;

    let portfolio = Portfolio::load(portfolio_config, assets, &converter, &mut quotes)?;
    print_portfolio(&portfolio);

    Ok(())
}

// FIXME: Implement + deduplicate code
pub fn rebalance(config: &Config, portfolio_name: &str) -> EmptyResult {
    let portfolio_config = config.get_portfolio(portfolio_name)?;
    let database = db::connect(&config.db_path)?;

    let converter = CurrencyConverter::new(database.clone(), false);
    let mut quotes = Quotes::new(&config, database.clone());

    let assets = Assets::load(database, &portfolio_config.name)?;
    assets.validate(&portfolio_config)?;

    let portfolio = Portfolio::load(portfolio_config, assets, &converter, &mut quotes)?;
    print_portfolio(&portfolio);

    Ok(())
}