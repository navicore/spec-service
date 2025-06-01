use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePool, Row};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::domain::{
    errors::DomainError,
    events::{SpecEvent, SpecState},
};

/// Read model for current spec state
#[derive(Debug, Clone)]
pub struct SpecProjection {
    pub id: Uuid,
    pub name: String,
    pub content: String,
    pub description: Option<String>,
    pub version: u32,
    pub state: SpecState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub updated_by: String,
}

/// Read model for spec summary (list views)
#[derive(Debug, Clone)]
pub struct SpecSummaryProjection {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub latest_version: u32,
    pub state: SpecState,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct ProjectionStore {
    pub(super) pool: SqlitePool,
    // In-memory cache for faster reads (optional optimization)
    cache: Arc<RwLock<Option<std::collections::HashMap<Uuid, SpecProjection>>>>,
}

impl ProjectionStore {
    pub async fn new(database_url: &str, enable_cache: bool) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        let cache = if enable_cache {
            Arc::new(RwLock::new(Some(std::collections::HashMap::new())))
        } else {
            Arc::new(RwLock::new(None))
        };

        Ok(Self { pool, cache })
    }

    pub async fn init_schema(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS spec_projections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                description TEXT,
                version INTEGER NOT NULL,
                state TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                created_by TEXT NOT NULL,
                updated_by TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_spec_projections_name 
            ON spec_projections(name);

            CREATE INDEX IF NOT EXISTS idx_spec_projections_state 
            ON spec_projections(state);

            CREATE INDEX IF NOT EXISTS idx_spec_projections_updated 
            ON spec_projections(updated_at DESC);

            -- Version history for querying specific versions
            CREATE TABLE IF NOT EXISTS spec_version_history (
                id TEXT NOT NULL,
                version INTEGER NOT NULL,
                content TEXT NOT NULL,
                description TEXT,
                created_at TEXT NOT NULL,
                created_by TEXT NOT NULL,
                PRIMARY KEY (id, version)
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Apply an event to update projections
    pub async fn apply_event(
        &self,
        _aggregate_id: Uuid,
        event: &SpecEvent,
    ) -> Result<(), DomainError> {
        match event {
            SpecEvent::Created(e) => self.handle_created(e).await,
            SpecEvent::Updated(e) => self.handle_updated(e).await,
            SpecEvent::StateChanged(e) => self.handle_state_changed(e).await,
        }
    }

    async fn handle_created(
        &self,
        event: &crate::domain::events::SpecCreated,
    ) -> Result<(), DomainError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        // Insert into main projection
        sqlx::query(
            r#"
            INSERT INTO spec_projections (
                id, name, content, description, version, state,
                created_at, updated_at, created_by, updated_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(event.spec_id.to_string())
        .bind(&event.name)
        .bind(&event.content)
        .bind(&event.description)
        .bind(1) // Initial version
        .bind("draft") // Initial state
        .bind(event.created_at.to_rfc3339())
        .bind(event.created_at.to_rfc3339())
        .bind(&event.created_by)
        .bind(&event.created_by)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        // Insert into version history
        sqlx::query(
            r#"
            INSERT INTO spec_version_history (
                id, version, content, description, created_at, created_by
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(event.spec_id.to_string())
        .bind(1)
        .bind(&event.content)
        .bind(&event.description)
        .bind(event.created_at.to_rfc3339())
        .bind(&event.created_by)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        // Update cache if enabled
        if let Some(cache) = self.cache.write().await.as_mut() {
            cache.insert(
                event.spec_id,
                SpecProjection {
                    id: event.spec_id,
                    name: event.name.clone(),
                    content: event.content.clone(),
                    description: event.description.clone(),
                    version: 1,
                    state: SpecState::Draft,
                    created_at: event.created_at,
                    updated_at: event.created_at,
                    created_by: event.created_by.clone(),
                    updated_by: event.created_by.clone(),
                },
            );
        }

        Ok(())
    }

    async fn handle_updated(
        &self,
        event: &crate::domain::events::SpecUpdated,
    ) -> Result<(), DomainError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        // Update main projection
        sqlx::query(
            r#"
            UPDATE spec_projections
            SET content = ?, description = ?, version = ?, 
                updated_at = ?, updated_by = ?
            WHERE id = ?
            "#,
        )
        .bind(&event.content)
        .bind(&event.description)
        .bind(event.version as i64)
        .bind(event.updated_at.to_rfc3339())
        .bind(&event.updated_by)
        .bind(event.spec_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        // Insert into version history
        sqlx::query(
            r#"
            INSERT INTO spec_version_history (
                id, version, content, description, created_at, created_by
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(event.spec_id.to_string())
        .bind(event.version as i64)
        .bind(&event.content)
        .bind(&event.description)
        .bind(event.updated_at.to_rfc3339())
        .bind(&event.updated_by)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        // Update cache if enabled
        if let Some(cache) = self.cache.write().await.as_mut() {
            if let Some(proj) = cache.get_mut(&event.spec_id) {
                proj.content = event.content.clone();
                proj.description = event.description.clone();
                proj.version = event.version;
                proj.updated_at = event.updated_at;
                proj.updated_by = event.updated_by.clone();
            }
        }

        Ok(())
    }

    async fn handle_state_changed(
        &self,
        event: &crate::domain::events::SpecStateChanged,
    ) -> Result<(), DomainError> {
        let state_str = match event.to_state {
            SpecState::Draft => "draft",
            SpecState::Published => "published",
            SpecState::Deprecated => "deprecated",
            SpecState::Deleted => "deleted",
        };

        sqlx::query(
            r#"
            UPDATE spec_projections
            SET state = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(state_str)
        .bind(event.changed_at.to_rfc3339())
        .bind(event.spec_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        // Update cache if enabled
        if let Some(cache) = self.cache.write().await.as_mut() {
            if let Some(proj) = cache.get_mut(&event.spec_id) {
                proj.state = event.to_state;
                proj.updated_at = event.changed_at;
            }
        }

        Ok(())
    }

    // Query methods for read models

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<SpecProjection>, DomainError> {
        // Check cache first
        if let Some(cache) = self.cache.read().await.as_ref() {
            if let Some(proj) = cache.get(&id) {
                return Ok(Some(proj.clone()));
            }
        }

        let row = sqlx::query(
            r#"
            SELECT id, name, content, description, version, state,
                   created_at, updated_at, created_by, updated_by
            FROM spec_projections
            WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(self.row_to_projection(row)?)),
            None => Ok(None),
        }
    }

    #[allow(dead_code)]
    pub async fn get_by_name(&self, name: &str) -> Result<Option<SpecProjection>, DomainError> {
        let row = sqlx::query(
            r#"
            SELECT id, name, content, description, version, state,
                   created_at, updated_at, created_by, updated_by
            FROM spec_projections
            WHERE name = ?
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        match row {
            Some(row) => Ok(Some(self.row_to_projection(row)?)),
            None => Ok(None),
        }
    }

    pub async fn list_by_state(
        &self,
        state: Option<SpecState>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SpecSummaryProjection>, DomainError> {
        let query = match state {
            Some(s) => {
                let state_str = match s {
                    SpecState::Draft => "draft",
                    SpecState::Published => "published",
                    SpecState::Deprecated => "deprecated",
                    SpecState::Deleted => "deleted",
                };
                sqlx::query(
                    r#"
                    SELECT id, name, description, version, state, updated_at
                    FROM spec_projections
                    WHERE state = ?
                    ORDER BY updated_at DESC
                    LIMIT ? OFFSET ?
                    "#,
                )
                .bind(state_str)
                .bind(limit)
                .bind(offset)
            }
            None => sqlx::query(
                r#"
                    SELECT id, name, description, version, state, updated_at
                    FROM spec_projections
                    WHERE state != 'deleted'
                    ORDER BY updated_at DESC
                    LIMIT ? OFFSET ?
                    "#,
            )
            .bind(limit)
            .bind(offset),
        };

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        let summaries = rows
            .into_iter()
            .map(|row| self.row_to_summary(row))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(summaries)
    }

    pub async fn get_version(
        &self,
        id: Uuid,
        version: u32,
    ) -> Result<Option<(String, Option<String>)>, DomainError> {
        let row = sqlx::query(
            r#"
            SELECT content, description
            FROM spec_version_history
            WHERE id = ? AND version = ?
            "#,
        )
        .bind(id.to_string())
        .bind(version as i64)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        match row {
            Some(row) => {
                let content: String = row.get("content");
                let description: Option<String> = row.get("description");
                Ok(Some((content, description)))
            }
            None => Ok(None),
        }
    }

    fn row_to_projection(
        &self,
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<SpecProjection, DomainError> {
        let id_str: String = row.get("id");
        let state_str: String = row.get("state");
        let created_at_str: String = row.get("created_at");
        let updated_at_str: String = row.get("updated_at");

        let state = match state_str.as_str() {
            "draft" => SpecState::Draft,
            "published" => SpecState::Published,
            "deprecated" => SpecState::Deprecated,
            "deleted" => SpecState::Deleted,
            _ => return Err(DomainError::ProjectionError("Invalid state".to_string())),
        };

        Ok(SpecProjection {
            id: Uuid::parse_str(&id_str)
                .map_err(|e| DomainError::ProjectionError(e.to_string()))?,
            name: row.get("name"),
            content: row.get("content"),
            description: row.get("description"),
            version: row.get::<i64, _>("version") as u32,
            state,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .map_err(|e| DomainError::ProjectionError(e.to_string()))?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| DomainError::ProjectionError(e.to_string()))?
                .with_timezone(&Utc),
            created_by: row.get("created_by"),
            updated_by: row.get("updated_by"),
        })
    }

    fn row_to_summary(
        &self,
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<SpecSummaryProjection, DomainError> {
        let id_str: String = row.get("id");
        let state_str: String = row.get("state");
        let updated_at_str: String = row.get("updated_at");

        let state = match state_str.as_str() {
            "draft" => SpecState::Draft,
            "published" => SpecState::Published,
            "deprecated" => SpecState::Deprecated,
            "deleted" => SpecState::Deleted,
            _ => return Err(DomainError::ProjectionError("Invalid state".to_string())),
        };

        Ok(SpecSummaryProjection {
            id: Uuid::parse_str(&id_str)
                .map_err(|e| DomainError::ProjectionError(e.to_string()))?,
            name: row.get("name"),
            description: row.get("description"),
            latest_version: row.get::<i64, _>("version") as u32,
            state,
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| DomainError::ProjectionError(e.to_string()))?
                .with_timezone(&Utc),
        })
    }
}
