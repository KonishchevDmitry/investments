#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;
// FIXME: A temporary workaround for IntelliJ Rust plugin
//#[macro_use] extern crate rust_decimal_macros;

#[macro_use] pub mod core;
#[macro_use] pub mod types;
pub mod analyse;
pub mod broker_statement;
pub mod brokers;
pub mod config;
pub mod currency;
pub mod db;
pub mod formatting;
pub mod portfolio;
pub mod quotes;
pub mod localities;
pub mod tax_statement;
pub mod util;