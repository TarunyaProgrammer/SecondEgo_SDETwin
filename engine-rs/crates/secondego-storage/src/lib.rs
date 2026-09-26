use std::path::Path;

use rusqlite::{Connection, params};
use secondego_core::{EngineEvent, ExecutionState};

#[derive(Debug)]
pub enum StorageError {
    Sqlite(rusqlite::Error),
    Serialization(serde_json::Error),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for StorageError {}
impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}
impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

pub struct SQLiteRunStore {
    connection: Connection,
}

impl SQLiteRunStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_memory() -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory()?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn save(&self, state: &ExecutionState, events: &[EngineEvent]) -> Result<(), StorageError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO runs (run_id, status, state_json) VALUES (?1, ?2, ?3)",
            params![
                state.run_id.to_string(),
                format!("{:?}", state.status),
                serde_json::to_string(state)?
            ],
        )?;
        transaction.execute(
            "DELETE FROM events WHERE run_id = ?1",
            params![state.run_id.to_string()],
        )?;
        for (sequence, event) in events.iter().enumerate() {
            transaction.execute(
                "INSERT INTO events (run_id, sequence, event_json) VALUES (?1, ?2, ?3)",
                params![
                    event.run_id.to_string(),
                    sequence as i64,
                    serde_json::to_string(event)?
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn event_count(&self, run_id: &str) -> Result<u32, StorageError> {
        Ok(self.connection.query_row(
            "SELECT COUNT(*) FROM events WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?)
    }

    fn migrate(&self) -> Result<(), StorageError> {
        self.connection.execute_batch("PRAGMA foreign_keys = ON; CREATE TABLE IF NOT EXISTS runs (run_id TEXT PRIMARY KEY, status TEXT NOT NULL, state_json TEXT NOT NULL); CREATE TABLE IF NOT EXISTS events (id INTEGER PRIMARY KEY AUTOINCREMENT, run_id TEXT NOT NULL REFERENCES runs(run_id) ON DELETE CASCADE, sequence INTEGER NOT NULL, event_json TEXT NOT NULL, UNIQUE(run_id, sequence));")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secondego_core::{ExecutionState, Phase, StateMachine};

    #[test]
    fn stores_final_state_and_versioned_events() {
        let store = SQLiteRunStore::open_memory().unwrap();
        let mut machine = StateMachine::new(ExecutionState::new("task", "."));
        let events = vec![machine.move_to(Phase::Understand, "test").unwrap()];
        store.save(&machine.state, &events).unwrap();
        assert_eq!(
            store
                .event_count(&machine.state.run_id.to_string())
                .unwrap(),
            1
        );
    }
}
