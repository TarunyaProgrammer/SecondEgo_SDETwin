import json
import sqlite3
from pathlib import Path
from typing import Sequence

from SecondEgo.context.policy import EvidenceRecord
from SecondEgo.core.events import EVENT_SCHEMA_VERSION, EngineEvent
from SecondEgo.core.state import ExecutionState

from .redaction import redact_sensitive


class SQLiteRunStore:
    """Versioned local persistence for compact execution evidence."""

    SCHEMA_VERSION = 1

    def __init__(self, database_path: Path) -> None:
        self.database_path = database_path.expanduser()
        self.database_path.parent.mkdir(parents=True, exist_ok=True)
        self._initialize()

    def save_run(
        self,
        state: ExecutionState,
        events: Sequence[EngineEvent],
        evidence: Sequence[EvidenceRecord],
    ) -> None:
        with self._connect() as connection:
            connection.execute(
                """
                INSERT INTO runs (run_id, task, workspace, phase, status, termination_reason, state_json)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(run_id) DO UPDATE SET
                    phase = excluded.phase,
                    status = excluded.status,
                    termination_reason = excluded.termination_reason,
                    state_json = excluded.state_json
                """,
                (
                    state.run_id,
                    state.task,
                    state.workspace,
                    state.phase.value,
                    state.status.value,
                    state.termination_reason,
                    json.dumps(state.snapshot(), default=str),
                ),
            )
            connection.execute("DELETE FROM events WHERE run_id = ?", (state.run_id,))
            connection.execute("DELETE FROM evidence WHERE run_id = ?", (state.run_id,))
            connection.executemany(
                """
                INSERT INTO events (run_id, sequence, event_type, phase, status, timestamp, evidence_ref, payload_json)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                [
                    (
                        state.run_id,
                        sequence,
                        event.event_type,
                        event.phase,
                        event.status,
                        event.timestamp.isoformat(),
                        event.evidence_ref,
                        json.dumps(event.payload, default=str),
                    )
                    for sequence, event in enumerate(events)
                ],
            )
            connection.executemany(
                """
                INSERT INTO evidence (run_id, reference, summary, source, importance, stale)
                VALUES (?, ?, ?, ?, ?, ?)
                """,
                [
                    (
                        state.run_id,
                        record.reference,
                        redact_sensitive(record.summary[:4_000]),
                        record.source,
                        record.importance,
                        int(record.stale),
                    )
                    for record in evidence
                ],
            )

    def load_run(self, run_id: str) -> dict[str, object] | None:
        with self._connect() as connection:
            run = connection.execute(
                "SELECT task, workspace, phase, status, termination_reason, state_json FROM runs WHERE run_id = ?",
                (run_id,),
            ).fetchone()
            if run is None:
                return None
            events = connection.execute(
                "SELECT event_type, phase, status, timestamp, evidence_ref, payload_json FROM events WHERE run_id = ? ORDER BY sequence",
                (run_id,),
            ).fetchall()
            evidence = connection.execute(
                "SELECT reference, summary, source, importance, stale FROM evidence WHERE run_id = ? ORDER BY reference",
                (run_id,),
            ).fetchall()
        return {
            "run_id": run_id,
            "task": run[0],
            "workspace": run[1],
            "phase": run[2],
            "status": run[3],
            "termination_reason": run[4],
            "state": json.loads(run[5]),
            "events": [
                {
                    "schema_version": EVENT_SCHEMA_VERSION,
                    "event_type": event[0],
                    "phase": event[1],
                    "status": event[2],
                    "timestamp": event[3],
                    "evidence_ref": event[4],
                    "payload": json.loads(event[5]),
                }
                for event in events
            ],
            "evidence": [
                {
                    "reference": record[0],
                    "summary": record[1],
                    "source": record[2],
                    "importance": record[3],
                    "stale": bool(record[4]),
                }
                for record in evidence
            ],
        }

    def _initialize(self) -> None:
        with self._connect() as connection:
            version = connection.execute("PRAGMA user_version").fetchone()[0]
            if version not in (0, self.SCHEMA_VERSION):
                raise RuntimeError(f"unsupported database schema version: {version}")
            connection.executescript(
                """
                CREATE TABLE IF NOT EXISTS runs (
                    run_id TEXT PRIMARY KEY,
                    task TEXT NOT NULL,
                    workspace TEXT NOT NULL,
                    phase TEXT NOT NULL,
                    status TEXT NOT NULL,
                    termination_reason TEXT,
                    state_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS events (
                    run_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL,
                    event_type TEXT NOT NULL,
                    phase TEXT NOT NULL,
                    status TEXT,
                    timestamp TEXT NOT NULL,
                    evidence_ref TEXT,
                    payload_json TEXT NOT NULL,
                    PRIMARY KEY (run_id, sequence)
                );
                CREATE TABLE IF NOT EXISTS evidence (
                    run_id TEXT NOT NULL,
                    reference TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    source TEXT NOT NULL,
                    importance INTEGER NOT NULL,
                    stale INTEGER NOT NULL,
                    PRIMARY KEY (run_id, reference)
                );
                """
            )
            connection.execute(f"PRAGMA user_version = {self.SCHEMA_VERSION}")

    def _connect(self) -> sqlite3.Connection:
        return sqlite3.connect(self.database_path)
