use anyhow::Result;
use chrono::Utc;
use sqlx::{sqlite::SqlitePool, Row};
use uuid::Uuid;

use crate::domain::{
    errors::DomainError,
    events::{EventEnvelope, EventMetadata, SpecEvent},
};

#[derive(Clone)]
pub struct SqliteEventStore {
    pool: SqlitePool,
}

impl SqliteEventStore {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            "
            CREATE TABLE IF NOT EXISTS events (
                event_id TEXT PRIMARY KEY,
                aggregate_id TEXT NOT NULL,
                sequence_number INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                event_data TEXT NOT NULL,
                metadata TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_events_aggregate_id 
            ON events(aggregate_id);

            CREATE UNIQUE INDEX IF NOT EXISTS idx_events_aggregate_sequence 
            ON events(aggregate_id, sequence_number);

            CREATE TABLE IF NOT EXISTS snapshots (
                aggregate_id TEXT PRIMARY KEY,
                sequence_number INTEGER NOT NULL,
                aggregate_data TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            ",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn append_events(
        &self,
        aggregate_id: Uuid,
        events: Vec<SpecEvent>,
        metadata: EventMetadata,
    ) -> Result<Vec<EventEnvelope>, DomainError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

        let last_sequence = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(sequence_number), 0) FROM events WHERE aggregate_id = ?",
        )
        .bind(aggregate_id.to_string())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

        let mut envelopes = Vec::new();
        let now = Utc::now();

        for (i, event) in events.into_iter().enumerate() {
            let event_id = Uuid::new_v4();
            let sequence_number = last_sequence + i64::try_from(i).unwrap_or(0) + 1;

            let event_type = match &event {
                SpecEvent::Created(_) => "created",
                SpecEvent::Updated(_) => "updated",
                SpecEvent::StateChanged(_) => "state_changed",
            };

            let event_data = serde_json::to_string(&event)
                .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

            let metadata_json = serde_json::to_string(&metadata)
                .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

            sqlx::query(
                "
                INSERT INTO events (
                    event_id, aggregate_id, sequence_number, 
                    event_type, event_data, metadata, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                ",
            )
            .bind(event_id.to_string())
            .bind(aggregate_id.to_string())
            .bind(sequence_number)
            .bind(event_type)
            .bind(&event_data)
            .bind(&metadata_json)
            .bind(now.to_rfc3339())
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

            envelopes.push(EventEnvelope {
                event_id,
                aggregate_id,
                sequence_number,
                event,
                metadata: metadata.clone(),
            });
        }

        tx.commit()
            .await
            .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

        Ok(envelopes)
    }

    pub async fn get_events(
        &self,
        aggregate_id: Uuid,
        from_sequence: Option<i64>,
    ) -> Result<Vec<EventEnvelope>, DomainError> {
        let from_sequence = from_sequence.unwrap_or(0);

        let rows = sqlx::query(
            "
            SELECT event_id, sequence_number, event_data, metadata
            FROM events
            WHERE aggregate_id = ? AND sequence_number > ?
            ORDER BY sequence_number
            ",
        )
        .bind(aggregate_id.to_string())
        .bind(from_sequence)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

        let mut envelopes = Vec::new();

        for row in rows {
            let event_id: String = row.get("event_id");
            let sequence_number: i64 = row.get("sequence_number");
            let event_data: String = row.get("event_data");
            let metadata_json: String = row.get("metadata");

            let event: SpecEvent = serde_json::from_str(&event_data)
                .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

            let metadata: EventMetadata = serde_json::from_str(&metadata_json)
                .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

            envelopes.push(EventEnvelope {
                event_id: Uuid::parse_str(&event_id)
                    .map_err(|e| DomainError::EventStoreError(e.to_string()))?,
                aggregate_id,
                sequence_number,
                event,
                metadata,
            });
        }

        Ok(envelopes)
    }

    pub async fn get_all_events(
        &self,
        from_global_sequence: i64,
        limit: i64,
    ) -> Result<Vec<(Uuid, EventEnvelope)>, DomainError> {
        let rows = sqlx::query(
            "
            SELECT rowid, event_id, aggregate_id, sequence_number, event_data, metadata
            FROM events
            WHERE rowid > ?
            ORDER BY rowid
            LIMIT ?
            ",
        )
        .bind(from_global_sequence)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

        let mut results = Vec::new();

        for row in rows {
            let event_id: String = row.get("event_id");
            let aggregate_id: String = row.get("aggregate_id");
            let sequence_number: i64 = row.get("sequence_number");
            let event_data: String = row.get("event_data");
            let metadata_json: String = row.get("metadata");

            let event: SpecEvent = serde_json::from_str(&event_data)
                .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

            let metadata: EventMetadata = serde_json::from_str(&metadata_json)
                .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

            let aggregate_id = Uuid::parse_str(&aggregate_id)
                .map_err(|e| DomainError::EventStoreError(e.to_string()))?;

            let envelope = EventEnvelope {
                event_id: Uuid::parse_str(&event_id)
                    .map_err(|e| DomainError::EventStoreError(e.to_string()))?,
                aggregate_id,
                sequence_number,
                event,
                metadata,
            };

            results.push((aggregate_id, envelope));
        }

        Ok(results)
    }
}
