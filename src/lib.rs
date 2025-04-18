#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;
#[macro_use] extern crate maplit;
#[macro_use] extern crate separator;

#[macro_use] pub mod core;
#[macro_use] pub mod types;

pub mod analysis;
pub mod cash_flow;
pub mod config;
pub mod db;
pub mod deposits;
pub mod metrics;
pub mod portfolio;
pub mod tax_statement;
pub mod telemetry;
pub mod time;
pub mod util;

mod broker_statement;
mod brokers;
mod commissions;
mod currency;
mod exchanges;
mod forex;
mod formats;
mod formatting;
mod instruments;
mod localities;
mod proto;
mod quotes;
mod rate_limiter;
mod taxes;
mod trades;