pub mod models;
pub mod schema;

use std::sync::{Arc, Mutex, MutexGuard};

use diesel::{Connection as ConnectionTrait, SqliteConnection};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness};
#[cfg(test)] use tempfile::NamedTempFile;

use crate::core::GenericResult;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[derive(Clone)]
pub struct Connection(Arc<Mutex<SqliteConnection>>);

impl Connection {
    pub fn borrow(&self) -> MutexGuard<'_, SqliteConnection> {
        self.0.as_ref().lock().unwrap()
    }
}

pub fn connect(url: &str) -> GenericResult<Connection> {
    let mut connection = SqliteConnection::establish(url).map_err(|e| format!(
        "Unable to open {url:?} database: {e}"))?;

    connection.run_pending_migrations(MIGRATIONS).map_err(|e| format!(
        "Failed to prepare the database: {e}"))?;

    Ok(Connection(Arc::new(Mutex::new(connection))))
}

#[cfg(test)]
pub fn new_temporary() -> (NamedTempFile, Connection) {
    let database = NamedTempFile::new().unwrap();
    let connection = connect(database.path().to_str().unwrap()).unwrap();
    (database, connection)
}