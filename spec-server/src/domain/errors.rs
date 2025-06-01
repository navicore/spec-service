use thiserror::Error;
use uuid::Uuid;

use super::events::SpecState;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Spec not found: {0}")]
    SpecNotFound(Uuid),
    
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: SpecState, to: SpecState },
    
    #[error("Version mismatch: expected {expected}, got {actual}")]
    VersionMismatch { expected: u32, actual: u32 },
    
    #[error("Spec already exists with name: {0}")]
    DuplicateSpecName(String),
    
    #[error("Cannot modify spec in {0:?} state")]
    InvalidStateForOperation(SpecState),
    
    #[error("Validation error: {0}")]
    ValidationError(#[from] super::value_objects::ValidationError),
    
    #[error("Event store error: {0}")]
    EventStoreError(String),
    
    #[error("Projection error: {0}")]
    ProjectionError(String),
}