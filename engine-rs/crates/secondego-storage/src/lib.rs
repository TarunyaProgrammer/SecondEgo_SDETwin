use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use secondego_core::{EngineEvent, ExecutionState};

pub const STORAGE_SCHEMA_VERSION: i64 = 3;

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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunMetadata {
    pub profile_id: Option<String>,
    pub workspace_id: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub engine_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StoredRun {
    pub state: ExecutionState,
    pub events: Vec<EngineEvent>,
    pub metadata: RunMetadata,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
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
        self.save_with_metadata(state, events, &RunMetadata::default())
    }

    pub fn save_with_metadata(
        &self,
        state: &ExecutionState,
        events: &[EngineEvent],
        metadata: &RunMetadata,
    ) -> Result<(), StorageError> {
        self.save_snapshot(state, events, metadata, None, None)
    }

    pub fn save_snapshot(
        &self,
        state: &ExecutionState,
        events: &[EngineEvent],
        metadata: &RunMetadata,
        result: Option<&serde_json::Value>,
        error: Option<&str>,
    ) -> Result<(), StorageError> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO runs (run_id, status, state_json, profile_id, workspace_id, provider, model, engine_version, created_at, updated_at, result_json, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now'), ?9, ?10)
             ON CONFLICT(run_id) DO UPDATE SET
               status = excluded.status,
               state_json = excluded.state_json,
               profile_id = excluded.profile_id,
               workspace_id = excluded.workspace_id,
               provider = excluded.provider,
               model = excluded.model,
               engine_version = excluded.engine_version,
               updated_at = excluded.updated_at,
               result_json = excluded.result_json,
               error = excluded.error",
            params![
                state.run_id.to_string(),
                format!("{:?}", state.status),
                serde_json::to_string(state)?,
                metadata.profile_id,
                metadata.workspace_id,
                metadata.provider,
                metadata.model,
                metadata.engine_version,
                result.map(serde_json::to_string).transpose()?,
                error,
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

    pub fn schema_version(&self) -> Result<i64, StorageError> {
        Ok(self.connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn run_metadata(&self, run_id: &str) -> Result<RunMetadata, StorageError> {
        Ok(self.connection.query_row(
            "SELECT profile_id, workspace_id, provider, model, engine_version FROM runs WHERE run_id = ?1",
            params![run_id],
            |row| {
                Ok(RunMetadata {
                    profile_id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    provider: row.get(2)?,
                    model: row.get(3)?,
                    engine_version: row.get(4)?,
                })
            },
        )?)
    }

    pub fn load(&self, run_id: &str) -> Result<Option<StoredRun>, StorageError> {
        let row = self
            .connection
            .query_row(
                "SELECT state_json, profile_id, workspace_id, provider, model, engine_version, result_json, error FROM runs WHERE run_id = ?1",
                params![run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        RunMetadata {
                            profile_id: row.get(1)?,
                            workspace_id: row.get(2)?,
                            provider: row.get(3)?,
                            model: row.get(4)?,
                            engine_version: row.get(5)?,
                        },
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((state_json, metadata, result_json, error)) = row else {
            return Ok(None);
        };
        let state = serde_json::from_str(&state_json)?;
        let mut statement = self
            .connection
            .prepare("SELECT event_json FROM events WHERE run_id = ?1 ORDER BY sequence ASC")?;
        let events = statement
            .query_map(params![run_id], |row| row.get::<_, String>(0))?
            .map(|event| {
                event
                    .map_err(StorageError::from)
                    .and_then(|value| Ok(serde_json::from_str(&value)?))
            })
            .collect::<Result<Vec<EngineEvent>, StorageError>>()?;
        Ok(Some(StoredRun {
            state,
            events,
            metadata,
            result: result_json
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
            error,
        }))
    }

    fn migrate(&self) -> Result<(), StorageError> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS schema_migrations (
               version INTEGER PRIMARY KEY,
               applied_at TEXT NOT NULL
             );",
        )?;
        let current = self.schema_version()?;
        if current < 1 {
            self.connection.execute_batch(
                "CREATE TABLE IF NOT EXISTS runs (
                   run_id TEXT PRIMARY KEY,
                   status TEXT NOT NULL,
                   state_json TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS events (
                   id INTEGER PRIMARY KEY AUTOINCREMENT,
                   run_id TEXT NOT NULL REFERENCES runs(run_id) ON DELETE CASCADE,
                   sequence INTEGER NOT NULL,
                   event_json TEXT NOT NULL,
                   UNIQUE(run_id, sequence)
                 );
                 INSERT INTO schema_migrations(version, applied_at)
                   VALUES (1, datetime('now'));",
            )?;
        }
        let current = self.schema_version()?;
        if current < 2 {
            for column in [
                "profile_id",
                "workspace_id",
                "provider",
                "model",
                "engine_version",
                "created_at",
                "updated_at",
            ] {
                if !self.has_column("runs", column)? {
                    self.connection
                        .execute(&format!("ALTER TABLE runs ADD COLUMN {column} TEXT"), [])?;
                }
            }
            self.connection.execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (2, datetime('now'))",
                [],
            )?;
        }
        let current = self.schema_version()?;
        if current < 3 {
            for column in ["result_json", "error"] {
                if !self.has_column("runs", column)? {
                    self.connection
                        .execute(&format!("ALTER TABLE runs ADD COLUMN {column} TEXT"), [])?;
                }
            }
            self.connection.execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (3, datetime('now'))",
                [],
            )?;
        }
        Ok(())
    }

    fn has_column(&self, table: &str, column: &str) -> Result<bool, StorageError> {
        let mut statement = self
            .connection
            .prepare(&format!("PRAGMA table_info({table})"))?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            if row.get::<_, String>(1)? == column {
                return Ok(true);
            }
        }
        Ok(false)
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
        let metadata = RunMetadata {
            profile_id: Some("openai:work".into()),
            workspace_id: Some("workspace-1".into()),
            provider: Some("openai".into()),
            model: Some("gpt-5-mini".into()),
            engine_version: Some("0.1.0".into()),
        };
        store
            .save_snapshot(
                &machine.state,
                &events,
                &metadata,
                Some(&serde_json::json!({"verified": true})),
                None,
            )
            .unwrap();
        assert_eq!(
            store
                .event_count(&machine.state.run_id.to_string())
                .unwrap(),
            1
        );
        assert_eq!(store.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        assert_eq!(
            store
                .run_metadata(&machine.state.run_id.to_string())
                .unwrap(),
            metadata
        );
        let loaded = store
            .load(&machine.state.run_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(loaded.events.len(), 1);
        assert_eq!(loaded.result.unwrap()["verified"], true);
    }

    #[test]
    fn upgrades_the_legacy_schema_without_losing_runs() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE runs (run_id TEXT PRIMARY KEY, status TEXT NOT NULL, state_json TEXT NOT NULL);
                 CREATE TABLE events (id INTEGER PRIMARY KEY AUTOINCREMENT, run_id TEXT NOT NULL, sequence INTEGER NOT NULL, event_json TEXT NOT NULL);",
            )
            .unwrap();
        let store = SQLiteRunStore { connection };
        store.migrate().unwrap();
        assert_eq!(store.schema_version().unwrap(), STORAGE_SCHEMA_VERSION);
        assert!(store.has_column("runs", "profile_id").unwrap());
    }
}
