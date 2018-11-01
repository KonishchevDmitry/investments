use diesel::{Connection as ConnectionTrait};
#[cfg(test)] use tempfile::NamedTempFile;

use core::GenericResult;

pub mod models;
pub mod schema;

pub use diesel::SqliteConnection as Connection;

embed_migrations!();

pub fn connect(url: &str) -> GenericResult<Connection> {
    let connection = Connection::establish(url).map_err(|e| format!(
        "Unable to open {:?} database: {}", url, e))?;

    embedded_migrations::run(&connection).map_err(|e| format!(
        "Failed to prepare the database: {}", e))?;

    Ok(connection)
}

#[cfg(test)]
pub fn new_temporary() -> (NamedTempFile, Connection) {
    let database = NamedTempFile::new().unwrap();
    let connection = connect(database.path().to_str().unwrap()).unwrap();
    (database, connection)
}