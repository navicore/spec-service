use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpecEvent {
    Created(SpecCreated),
    Updated(SpecUpdated),
    StateChanged(SpecStateChanged),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecCreated {
    pub spec_id: Uuid,
    pub name: String,
    pub content: String,
    pub description: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecUpdated {
    pub spec_id: Uuid,
    pub version: u32,
    pub content: String,
    pub description: Option<String>,
    pub updated_by: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecStateChanged {
    pub spec_id: Uuid,
    pub version: u32,
    pub from_state: SpecState,
    pub to_state: SpecState,
    pub reason: Option<String>,
    pub changed_by: String,
    pub changed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpecState {
    Draft,
    Published,
    Deprecated,
    Deleted,
}

impl Default for SpecState {
    fn default() -> Self {
        Self::Draft
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: Uuid,
    pub aggregate_id: Uuid,
    pub sequence_number: i64,
    pub event: SpecEvent,
    pub metadata: EventMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventMetadata {
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}
