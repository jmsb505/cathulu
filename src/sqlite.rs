use rusqlite::types::ValueRef;
use rusqlite::Connection as SqliteConnection;
use std::path::Path;
use std::time::Duration;

pub(crate) struct Connection {
    inner: SqliteConnection,
}

impl Connection {
    pub fn open(path: &Path) -> Result<Self, String> {
        let inner = SqliteConnection::open(path)
            .map_err(|error| format!("cannot open Cathulu SQLite store: {error}"))?;
        inner
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| format!("cannot configure SQLite busy timeout: {error}"))?;
        inner
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .map_err(|error| format!("cannot configure SQLite store: {error}"))?;
        Ok(Self { inner })
    }

    pub fn execute(&self, sql: &str) -> Result<(), String> {
        self.inner
            .execute_batch(sql)
            .map_err(|error| format!("SQLite statement failed: {error}"))
    }

    pub fn query(&self, sql: &str) -> Result<Vec<Vec<Option<String>>>, String> {
        let mut statement = self
            .inner
            .prepare(sql)
            .map_err(|error| format!("SQLite query could not be prepared: {error}"))?;
        let column_count = statement.column_count();
        let mut cursor = statement
            .query([])
            .map_err(|error| format!("SQLite query failed: {error}"))?;
        let mut result = Vec::new();
        while let Some(row) = cursor
            .next()
            .map_err(|error| format!("SQLite query failed: {error}"))?
        {
            let mut values = Vec::with_capacity(column_count);
            for index in 0..column_count {
                let value = row
                    .get_ref(index)
                    .map_err(|error| format!("SQLite result could not be read: {error}"))?;
                values.push(match value {
                    ValueRef::Null => None,
                    ValueRef::Integer(value) => Some(value.to_string()),
                    ValueRef::Real(value) => Some(value.to_string()),
                    ValueRef::Text(value) | ValueRef::Blob(value) => {
                        Some(String::from_utf8_lossy(value).into_owned())
                    }
                });
            }
            result.push(values);
        }
        Ok(result)
    }

    pub(crate) fn raw(&self) -> &SqliteConnection {
        &self.inner
    }

    pub(crate) fn raw_mut(&mut self) -> &mut SqliteConnection {
        &mut self.inner
    }
}
