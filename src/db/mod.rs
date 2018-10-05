use diesel::{Connection as ConnectionTrait};

use core::GenericResult;

pub mod models;
pub mod schema;

pub use diesel::SqliteConnection as Connection;

embed_migrations!();

pub fn connect(url: &str) -> GenericResult<Connection> {
    let connection = Connection::establish(url).map_err(|e| format!(
        "Unable to connect to {:?} database: {}", url, e))?;

    embedded_migrations::run(&connection).map_err(|e| format!(
        "Failed to prepare the database: {}", e))?;

    Ok(connection)
}