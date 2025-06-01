use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::domain::{
    aggregates::Spec,
    commands::{CreateSpec, DeprecateSpec, PublishSpec, UpdateSpec},
    errors::DomainError,
    events::{EventMetadata, SpecEvent, SpecState},
};
use crate::infrastructure::{
    event_store::SqliteEventStore,
    projections::{ProjectionStore, SpecProjection, SpecSummaryProjection},
};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub event_store: Arc<SqliteEventStore>,
    pub projection_store: Arc<ProjectionStore>,
}

/// Request/Response DTOs

#[derive(Debug, Deserialize)]
pub struct CreateSpecRequest {
    pub name: String,
    pub content: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateSpecResponse {
    pub id: Uuid,
    pub version: u32,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSpecRequest {
    pub content: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateSpecResponse {
    pub version: u32,
}

#[derive(Debug, Deserialize)]
pub struct PublishSpecRequest {
    pub version: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct DeprecateSpecRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct ListSpecsQuery {
    pub state: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SpecResponse {
    pub id: Uuid,
    pub name: String,
    pub content: String,
    pub description: Option<String>,
    pub version: u32,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    pub created_by: String,
    pub updated_by: String,
}

#[derive(Debug, Serialize)]
pub struct SpecSummaryResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub latest_version: u32,
    pub state: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ListSpecsResponse {
    pub specs: Vec<SpecSummaryResponse>,
    pub total: usize,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

/// Create the REST API router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/specs", post(create_spec).get(list_specs))
        .route("/specs/:id", get(get_spec).put(update_spec))
        .route("/specs/:id/publish", post(publish_spec))
        .route("/specs/:id/deprecate", post(deprecate_spec))
        .route("/specs/:id/versions/:version", get(get_spec_version))
        .route("/health", get(health_check))
        .with_state(state)
}

// Handler functions

async fn create_spec(
    State(state): State<AppState>,
    Json(req): Json<CreateSpecRequest>,
) -> Result<(StatusCode, Json<CreateSpecResponse>), (StatusCode, Json<ErrorResponse>)> {
    // TODO: Extract user from auth context
    let user = "user@example.com";

    let command = CreateSpec {
        name: req.name,
        content: req.content,
        description: req.description,
        created_by: user.to_string(),
    };

    let events = Spec::create(command).map_err(|e| handle_domain_error(&e))?;

    let spec_id = match &events[0] {
        SpecEvent::Created(e) => e.spec_id,
        _ => unreachable!(),
    };

    let metadata = EventMetadata {
        correlation_id: Some(Uuid::new_v4()),
        causation_id: None,
        user_agent: None, // TODO: Extract from headers
        ip_address: None, // TODO: Extract from connection
    };

    state
        .event_store
        .append_events(spec_id, events, metadata)
        .await
        .map_err(|e| handle_domain_error(&e))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateSpecResponse {
            id: spec_id,
            version: 1,
        }),
    ))
}

async fn get_spec(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SpecResponse>, (StatusCode, Json<ErrorResponse>)> {
    let spec = state
        .projection_store
        .get_by_id(id)
        .await
        .map_err(|e| handle_domain_error(&e))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Spec not found".to_string(),
                    details: None,
                }),
            )
        })?;

    Ok(Json(projection_to_response(spec)))
}

async fn update_spec(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateSpecRequest>,
) -> Result<Json<UpdateSpecResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Load current state from events
    let events = state
        .event_store
        .get_events(id, None)
        .await
        .map_err(|e| handle_domain_error(&e))?;

    if events.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Spec not found".to_string(),
                details: None,
            }),
        ));
    }

    let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())
        .map_err(|e| handle_domain_error(&e))?;

    let user = "user@example.com"; // TODO: From auth

    let command = UpdateSpec {
        spec_id: id,
        content: req.content,
        description: req.description,
        updated_by: user.to_string(),
    };

    let new_events = spec
        .handle_command(command.into())
        .map_err(|e| handle_domain_error(&e))?;

    let new_version = match &new_events[0] {
        SpecEvent::Updated(e) => e.version,
        _ => unreachable!(),
    };

    state
        .event_store
        .append_events(id, new_events, EventMetadata::default())
        .await
        .map_err(|e| handle_domain_error(&e))?;

    Ok(Json(UpdateSpecResponse {
        version: new_version,
    }))
}

async fn publish_spec(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<PublishSpecRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let events = state
        .event_store
        .get_events(id, None)
        .await
        .map_err(|e| handle_domain_error(&e))?;

    if events.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Spec not found".to_string(),
                details: None,
            }),
        ));
    }

    let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())
        .map_err(|e| handle_domain_error(&e))?;

    let user = "admin@example.com"; // TODO: From auth, check permissions

    let command = PublishSpec {
        spec_id: id,
        version: req.version,
        published_by: user.to_string(),
    };

    let new_events = spec
        .handle_command(command.into())
        .map_err(|e| handle_domain_error(&e))?;

    state
        .event_store
        .append_events(id, new_events, EventMetadata::default())
        .await
        .map_err(|e| handle_domain_error(&e))?;

    Ok(StatusCode::OK)
}

async fn deprecate_spec(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<DeprecateSpecRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let events = state
        .event_store
        .get_events(id, None)
        .await
        .map_err(|e| handle_domain_error(&e))?;

    if events.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Spec not found".to_string(),
                details: None,
            }),
        ));
    }

    let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())
        .map_err(|e| handle_domain_error(&e))?;

    let user = "admin@example.com"; // TODO: From auth, check permissions

    let command = DeprecateSpec {
        spec_id: id,
        reason: req.reason,
        deprecated_by: user.to_string(),
    };

    let new_events = spec
        .handle_command(command.into())
        .map_err(|e| handle_domain_error(&e))?;

    state
        .event_store
        .append_events(id, new_events, EventMetadata::default())
        .await
        .map_err(|e| handle_domain_error(&e))?;

    Ok(StatusCode::OK)
}

async fn list_specs(
    State(state): State<AppState>,
    Query(query): Query<ListSpecsQuery>,
) -> Result<Json<ListSpecsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let state_filter = match query.state.as_deref() {
        Some("draft") => Some(SpecState::Draft),
        Some("published") => Some(SpecState::Published),
        Some("deprecated") => Some(SpecState::Deprecated),
        Some("deleted") => Some(SpecState::Deleted),
        _ => None,
    };

    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);

    let specs = state
        .projection_store
        .list_by_state(state_filter, limit, offset)
        .await
        .map_err(|e| handle_domain_error(&e))?;

    let total = specs.len();

    Ok(Json(ListSpecsResponse {
        specs: specs.into_iter().map(summary_to_response).collect(),
        total,
        limit,
        offset,
    }))
}

async fn get_spec_version(
    State(state): State<AppState>,
    Path((id, version)): Path<(Uuid, u32)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let (content, description) = state
        .projection_store
        .get_version(id, version)
        .await
        .map_err(|e| handle_domain_error(&e))?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Version not found".to_string(),
                    details: None,
                }),
            )
        })?;

    Ok(Json(serde_json::json!({
        "id": id,
        "version": version,
        "content": content,
        "description": description,
    })))
}

async fn health_check() -> StatusCode {
    StatusCode::OK
}

// Helper functions

fn handle_domain_error(error: &DomainError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, message) = match error {
        DomainError::SpecNotFound(_) => (StatusCode::NOT_FOUND, "Spec not found"),
        DomainError::InvalidStateTransition { .. } => {
            (StatusCode::BAD_REQUEST, "Invalid state transition")
        }
        DomainError::VersionMismatch { .. } => (StatusCode::CONFLICT, "Version mismatch"),
        DomainError::DuplicateSpecName(_) => (StatusCode::CONFLICT, "Spec name already exists"),
        DomainError::InvalidStateForOperation(_) => (
            StatusCode::BAD_REQUEST,
            "Invalid operation for current state",
        ),
        DomainError::ValidationError(_) => (StatusCode::BAD_REQUEST, "Validation failed"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
    };

    (
        status,
        Json(ErrorResponse {
            error: message.to_string(),
            details: Some(error.to_string()),
        }),
    )
}

fn projection_to_response(proj: SpecProjection) -> SpecResponse {
    SpecResponse {
        id: proj.id,
        name: proj.name,
        content: proj.content,
        description: proj.description,
        version: proj.version,
        state: format!("{:?}", proj.state).to_lowercase(),
        created_at: proj.created_at.to_rfc3339(),
        updated_at: proj.updated_at.to_rfc3339(),
        created_by: proj.created_by,
        updated_by: proj.updated_by,
    }
}

fn summary_to_response(summary: SpecSummaryProjection) -> SpecSummaryResponse {
    SpecSummaryResponse {
        id: summary.id,
        name: summary.name,
        description: summary.description,
        latest_version: summary.latest_version,
        state: format!("{:?}", summary.state).to_lowercase(),
        updated_at: summary.updated_at.to_rfc3339(),
    }
}

// Command conversion implementations
impl From<UpdateSpec> for crate::domain::commands::SpecCommand {
    fn from(cmd: UpdateSpec) -> Self {
        Self::Update(cmd)
    }
}

impl From<PublishSpec> for crate::domain::commands::SpecCommand {
    fn from(cmd: PublishSpec) -> Self {
        Self::Publish(cmd)
    }
}

impl From<DeprecateSpec> for crate::domain::commands::SpecCommand {
    fn from(cmd: DeprecateSpec) -> Self {
        Self::Deprecate(cmd)
    }
}
