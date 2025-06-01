use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::domain::{
    aggregates::Spec,
    commands::{CreateSpec, DeprecateSpec, PublishSpec, UpdateSpec},
    errors::DomainError,
    events::{EventMetadata, SpecEvent, SpecState},
};
use crate::infrastructure::{event_store::SqliteEventStore, projections::ProjectionStore};

// Import generated protobuf types
#[allow(clippy::pedantic, clippy::nursery, clippy::all)]
pub mod spec_proto {
    tonic::include_proto!("spec");
}

use spec_proto::{
    spec_service_server::{SpecService, SpecServiceServer},
    CreateSpecRequest, CreateSpecResponse, DeprecateSpecRequest, DeprecateSpecResponse, EventType,
    GetSpecHistoryRequest, GetSpecHistoryResponse, GetSpecRequest, GetSpecResponse,
    ListSpecsRequest, ListSpecsResponse, PublishSpecRequest, PublishSpecResponse,
    SpecEvent as ProtoSpecEvent, SpecState as ProtoSpecState, SpecSummary, UpdateSpecRequest,
    UpdateSpecResponse,
};

pub struct SpecServiceImpl {
    event_store: Arc<SqliteEventStore>,
    projection_store: Arc<ProjectionStore>,
}

impl SpecServiceImpl {
    pub fn new(event_store: Arc<SqliteEventStore>, projection_store: Arc<ProjectionStore>) -> Self {
        Self {
            event_store,
            projection_store,
        }
    }

    pub fn into_service(self) -> SpecServiceServer<Self> {
        SpecServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl SpecService for SpecServiceImpl {
    async fn create_spec(
        &self,
        request: Request<CreateSpecRequest>,
    ) -> Result<Response<CreateSpecResponse>, Status> {
        let req = request.into_inner();

        // TODO: Extract user from request metadata
        let user = "grpc-user@example.com";

        let command = CreateSpec {
            name: req.name,
            content: req.content,
            description: if req.description.is_empty() {
                None
            } else {
                Some(req.description)
            },
            created_by: user.to_string(),
        };

        let events = Spec::create(command)
            .map_err(|e| Status::invalid_argument(format!("Validation failed: {e}")))?;

        let spec_id = match &events[0] {
            SpecEvent::Created(e) => e.spec_id,
            _ => unreachable!(),
        };

        let metadata = EventMetadata {
            correlation_id: Some(Uuid::new_v4()),
            causation_id: None,
            user_agent: None,
            ip_address: None,
        };

        self.event_store
            .append_events(spec_id, events, metadata)
            .await
            .map_err(|e| Status::internal(format!("Failed to store events: {e}")))?;

        // Wait briefly for projections
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Ok(Response::new(CreateSpecResponse {
            id: spec_id.to_string(),
            version: 1,
        }))
    }

    async fn update_spec(
        &self,
        request: Request<UpdateSpecRequest>,
    ) -> Result<Response<UpdateSpecResponse>, Status> {
        let req = request.into_inner();
        let spec_id =
            Uuid::parse_str(&req.id).map_err(|_| Status::invalid_argument("Invalid spec ID"))?;

        // Load current state
        let events = self
            .event_store
            .get_events(spec_id, None)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if events.is_empty() {
            return Err(Status::not_found("Spec not found"));
        }

        let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())
            .map_err(|e| Status::internal(e.to_string()))?;

        let user = "grpc-user@example.com";

        let command = UpdateSpec {
            spec_id,
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

        self.event_store
            .append_events(spec_id, new_events, EventMetadata::default())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(UpdateSpecResponse {
            version: new_version,
        }))
    }

    async fn get_spec(
        &self,
        request: Request<GetSpecRequest>,
    ) -> Result<Response<GetSpecResponse>, Status> {
        let req = request.into_inner();
        let spec_id =
            Uuid::parse_str(&req.id).map_err(|_| Status::invalid_argument("Invalid spec ID"))?;

        let spec = if let Some(version) = req.version {
            // Get specific version from history
            let (content, description) = self
                .projection_store
                .get_version(spec_id, version)
                .await
                .map_err(|e| Status::internal(e.to_string()))?
                .ok_or_else(|| Status::not_found("Version not found"))?;

            // Also need current spec for metadata
            let current = self
                .projection_store
                .get_by_id(spec_id)
                .await
                .map_err(|e| Status::internal(e.to_string()))?
                .ok_or_else(|| Status::not_found("Spec not found"))?;

            GetSpecResponse {
                id: spec_id.to_string(),
                name: current.name,
                content,
                description: description.unwrap_or_default(),
                version,
                state: domain_state_to_proto(current.state) as i32,
                created_at: Some(chrono_to_proto_timestamp(current.created_at)),
                updated_at: Some(chrono_to_proto_timestamp(current.updated_at)),
            }
        } else {
            // Get current version
            let spec = self
                .projection_store
                .get_by_id(spec_id)
                .await
                .map_err(|e| Status::internal(e.to_string()))?
                .ok_or_else(|| Status::not_found("Spec not found"))?;

            GetSpecResponse {
                id: spec.id.to_string(),
                name: spec.name,
                content: spec.content,
                description: spec.description.unwrap_or_default(),
                version: spec.version,
                state: domain_state_to_proto(spec.state) as i32,
                created_at: Some(chrono_to_proto_timestamp(spec.created_at)),
                updated_at: Some(chrono_to_proto_timestamp(spec.updated_at)),
            }
        };

        Ok(Response::new(spec))
    }

    async fn list_specs(
        &self,
        request: Request<ListSpecsRequest>,
    ) -> Result<Response<ListSpecsResponse>, Status> {
        let req = request.into_inner();

        let state_filter = req
            .state
            .and_then(|s| ProtoSpecState::try_from(s).ok().map(proto_state_to_domain));

        let page_size = i64::from(req.page_size);
        let offset = 0; // TODO: Implement page token parsing

        let specs = self
            .projection_store
            .list_by_state(state_filter, page_size, offset)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let summaries: Vec<SpecSummary> = specs
            .into_iter()
            .map(|s| SpecSummary {
                id: s.id.to_string(),
                name: s.name,
                description: s.description.unwrap_or_default(),
                latest_version: s.latest_version,
                state: domain_state_to_proto(s.state) as i32,
                updated_at: Some(chrono_to_proto_timestamp(s.updated_at)),
            })
            .collect();

        Ok(Response::new(ListSpecsResponse {
            specs: summaries,
            next_page_token: String::new(), // TODO: Implement pagination
        }))
    }

    async fn publish_spec(
        &self,
        request: Request<PublishSpecRequest>,
    ) -> Result<Response<PublishSpecResponse>, Status> {
        let req = request.into_inner();
        let spec_id =
            Uuid::parse_str(&req.id).map_err(|_| Status::invalid_argument("Invalid spec ID"))?;

        let events = self
            .event_store
            .get_events(spec_id, None)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if events.is_empty() {
            return Err(Status::not_found("Spec not found"));
        }

        let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())
            .map_err(|e| Status::internal(e.to_string()))?;

        let user = "grpc-admin@example.com";

        let command = PublishSpec {
            spec_id,
            version: req.version,
            published_by: user.to_string(),
        };

        let new_events = spec
            .handle_command(command.into())
            .map_err(|e| handle_domain_error(&e))?;

        self.event_store
            .append_events(spec_id, new_events, EventMetadata::default())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(PublishSpecResponse {
            published_version: spec.version.as_u32(),
        }))
    }

    async fn deprecate_spec(
        &self,
        request: Request<DeprecateSpecRequest>,
    ) -> Result<Response<DeprecateSpecResponse>, Status> {
        let req = request.into_inner();
        let spec_id =
            Uuid::parse_str(&req.id).map_err(|_| Status::invalid_argument("Invalid spec ID"))?;

        let events = self
            .event_store
            .get_events(spec_id, None)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if events.is_empty() {
            return Err(Status::not_found("Spec not found"));
        }

        let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())
            .map_err(|e| Status::internal(e.to_string()))?;

        let user = "grpc-admin@example.com";

        let command = DeprecateSpec {
            spec_id,
            reason: req.reason,
            deprecated_by: user.to_string(),
        };

        let new_events = spec
            .handle_command(command.into())
            .map_err(|e| handle_domain_error(&e))?;

        self.event_store
            .append_events(spec_id, new_events, EventMetadata::default())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(DeprecateSpecResponse { success: true }))
    }

    async fn get_spec_history(
        &self,
        request: Request<GetSpecHistoryRequest>,
    ) -> Result<Response<GetSpecHistoryResponse>, Status> {
        let req = request.into_inner();
        let spec_id =
            Uuid::parse_str(&req.id).map_err(|_| Status::invalid_argument("Invalid spec ID"))?;

        let event_envelopes = self
            .event_store
            .get_events(spec_id, None)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let proto_events: Vec<ProtoSpecEvent> = event_envelopes
            .into_iter()
            .map(|envelope| {
                let (event_type, payload) = match &envelope.event {
                    SpecEvent::Created(e) => (
                        EventType::Created,
                        spec_proto::spec_event::Payload::Create(spec_proto::CreatePayload {
                            name: e.name.clone(),
                            content: e.content.clone(),
                            description: e.description.clone().unwrap_or_default(),
                        }),
                    ),
                    SpecEvent::Updated(e) => (
                        EventType::Updated,
                        spec_proto::spec_event::Payload::Update(spec_proto::UpdatePayload {
                            content: e.content.clone(),
                            description: e.description.clone(),
                        }),
                    ),
                    SpecEvent::StateChanged(e) => (
                        EventType::StateChanged,
                        spec_proto::spec_event::Payload::StateChange(
                            spec_proto::StateChangePayload {
                                from_state: domain_state_to_proto(e.from_state) as i32,
                                to_state: domain_state_to_proto(e.to_state) as i32,
                                reason: e.reason.clone(),
                            },
                        ),
                    ),
                };

                ProtoSpecEvent {
                    event_id: envelope.event_id.to_string(),
                    event_type: event_type as i32,
                    occurred_at: Some(chrono_to_proto_timestamp(get_event_timestamp(
                        &envelope.event,
                    ))),
                    user_id: get_event_user(&envelope.event),
                    payload: Some(payload),
                }
            })
            .collect();

        Ok(Response::new(GetSpecHistoryResponse {
            events: proto_events,
        }))
    }
}

// Helper functions

fn handle_domain_error(error: &DomainError) -> Status {
    match error {
        DomainError::SpecNotFound(_) => Status::not_found("Spec not found"),
        DomainError::InvalidStateTransition { .. } => {
            Status::failed_precondition(error.to_string())
        }
        DomainError::VersionMismatch { .. } => Status::aborted(error.to_string()),
        DomainError::DuplicateSpecName(_) => Status::already_exists(error.to_string()),
        DomainError::InvalidStateForOperation(_) => Status::failed_precondition(error.to_string()),
        DomainError::ValidationError(_) => Status::invalid_argument(error.to_string()),
        _ => Status::internal(error.to_string()),
    }
}

fn domain_state_to_proto(state: SpecState) -> ProtoSpecState {
    match state {
        SpecState::Draft => ProtoSpecState::Draft,
        SpecState::Published => ProtoSpecState::Published,
        SpecState::Deprecated => ProtoSpecState::Deprecated,
        SpecState::Deleted => ProtoSpecState::Deleted,
    }
}

fn proto_state_to_domain(state: ProtoSpecState) -> SpecState {
    match state {
        ProtoSpecState::Draft => SpecState::Draft,
        ProtoSpecState::Published => SpecState::Published,
        ProtoSpecState::Deprecated => SpecState::Deprecated,
        ProtoSpecState::Deleted => SpecState::Deleted,
    }
}

fn chrono_to_proto_timestamp(dt: chrono::DateTime<chrono::Utc>) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: dt.timestamp(),
        nanos: i32::try_from(dt.timestamp_subsec_nanos()).unwrap_or(0),
    }
}

fn get_event_timestamp(event: &SpecEvent) -> chrono::DateTime<chrono::Utc> {
    match event {
        SpecEvent::Created(e) => e.created_at,
        SpecEvent::Updated(e) => e.updated_at,
        SpecEvent::StateChanged(e) => e.changed_at,
    }
}

fn get_event_user(event: &SpecEvent) -> String {
    match event {
        SpecEvent::Created(e) => e.created_by.clone(),
        SpecEvent::Updated(e) => e.updated_by.clone(),
        SpecEvent::StateChanged(e) => e.changed_by.clone(),
    }
}

// Command conversion implementations are already in rest.rs
