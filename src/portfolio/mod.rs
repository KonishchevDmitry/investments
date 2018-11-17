use config::Config;
use core::EmptyResult;
use db;
use broker_statement::BrokerStatement;

use self::asset_allocation::AssetAllocation;
use self::assets::Assets;

mod asset_allocation;
mod assets;

// FIXME: flat mode
pub fn show(config: &Config, portfolio_name: &str) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let database = db::connect(&config.db_path)?;

    let assets = Assets::load(database, &portfolio.name)?;
    assets.validate(&portfolio)?;

    let asset_allocation = AssetAllocation::load(portfolio, &assets)?;

//    let converter = CurrencyConverter::new(database.clone(), false);
//    let mut quotes = Quotes::new(&config, database.clone());
    asset_allocation.print(0);

    Ok(())
}

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