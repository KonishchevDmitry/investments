use config::Config;
use core::EmptyResult;

use self::asset_allocation::Assets;

mod asset_allocation;

// FIXME: flat mode
pub fn show(config: &Config, portfolio_name: &str) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let assets = Assets::parse(portfolio)?;

//    let database = db::connect(&config.db_path)?;
//    let converter = CurrencyConverter::new(database.clone(), false);
//    let mut quotes = Quotes::new(&config, database.clone());
    assets.print(0);

    Ok(())
}