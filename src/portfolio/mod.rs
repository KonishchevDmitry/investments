use config::Config;
use core::EmptyResult;
use db;
use broker_statement::BrokerStatement;

use self::asset_allocation::Portfolio;
use self::assets::Assets;

mod asset_allocation;
mod assets;

pub fn show(config: &Config, portfolio_name: &str) -> EmptyResult {
    let portfolio_config = config.get_portfolio(portfolio_name)?;
    let database = db::connect(&config.db_path)?;

    let assets = Assets::load(database, &portfolio_config.name)?;
    assets.validate(&portfolio_config)?;

    let portfolio = Portfolio::load(portfolio_config, &assets)?;

//    let converter = CurrencyConverter::new(database.clone(), false);
//    let mut quotes = Quotes::new(&config, database.clone());
    portfolio.print();

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