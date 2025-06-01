use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{
    commands::*,
    errors::DomainError,
    events::{SpecCreated, SpecEvent, SpecState, SpecStateChanged, SpecUpdated},
    value_objects::{SpecContent, SpecName, Version},
};

#[derive(Debug, Clone)]
pub struct Spec {
    pub id: Uuid,
    pub name: SpecName,
    pub content: SpecContent,
    pub description: Option<String>,
    pub version: Version,
    pub state: SpecState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub updated_by: String,
}

impl Spec {
    pub fn handle_command(
        &self,
        command: SpecCommand,
    ) -> Result<Vec<SpecEvent>, DomainError> {
        match command {
            SpecCommand::Create(_) => Err(DomainError::DuplicateSpecName(self.name.to_string())),
            SpecCommand::Update(cmd) => self.handle_update(cmd),
            SpecCommand::Publish(cmd) => self.handle_publish(cmd),
            SpecCommand::Deprecate(cmd) => self.handle_deprecate(cmd),
            SpecCommand::Delete(cmd) => self.handle_delete(cmd),
        }
    }

    pub fn create(command: CreateSpec) -> Result<Vec<SpecEvent>, DomainError> {
        let spec_id = Uuid::new_v4();
        let name = SpecName::new(command.name)?;
        let content = SpecContent::new(command.content)?;
        let now = Utc::now();

        Ok(vec![SpecEvent::Created(SpecCreated {
            spec_id,
            name: name.as_str().to_string(),
            content: content.as_str().to_string(),
            description: command.description,
            created_by: command.created_by,
            created_at: now,
        })])
    }

    fn handle_update(&self, command: UpdateSpec) -> Result<Vec<SpecEvent>, DomainError> {
        if self.state == SpecState::Deleted {
            return Err(DomainError::InvalidStateForOperation(self.state));
        }

        let content = SpecContent::new(command.content)?;
        let now = Utc::now();

        Ok(vec![SpecEvent::Updated(SpecUpdated {
            spec_id: self.id,
            version: self.version.increment().as_u32(),
            content: content.as_str().to_string(),
            description: command.description,
            updated_by: command.updated_by,
            updated_at: now,
        })])
    }

    fn handle_publish(&self, command: PublishSpec) -> Result<Vec<SpecEvent>, DomainError> {
        if let Some(version) = command.version {
            if version != self.version.as_u32() {
                return Err(DomainError::VersionMismatch {
                    expected: self.version.as_u32(),
                    actual: version,
                });
            }
        }

        if self.state != SpecState::Draft {
            return Err(DomainError::InvalidStateTransition {
                from: self.state,
                to: SpecState::Published,
            });
        }

        Ok(vec![SpecEvent::StateChanged(SpecStateChanged {
            spec_id: self.id,
            version: self.version.as_u32(),
            from_state: self.state,
            to_state: SpecState::Published,
            reason: None,
            changed_by: command.published_by,
            changed_at: Utc::now(),
        })])
    }

    fn handle_deprecate(&self, command: DeprecateSpec) -> Result<Vec<SpecEvent>, DomainError> {
        if self.state != SpecState::Published {
            return Err(DomainError::InvalidStateTransition {
                from: self.state,
                to: SpecState::Deprecated,
            });
        }

        Ok(vec![SpecEvent::StateChanged(SpecStateChanged {
            spec_id: self.id,
            version: self.version.as_u32(),
            from_state: self.state,
            to_state: SpecState::Deprecated,
            reason: Some(command.reason),
            changed_by: command.deprecated_by,
            changed_at: Utc::now(),
        })])
    }

    fn handle_delete(&self, command: DeleteSpec) -> Result<Vec<SpecEvent>, DomainError> {
        if self.state == SpecState::Deleted {
            return Err(DomainError::InvalidStateForOperation(self.state));
        }

        Ok(vec![SpecEvent::StateChanged(SpecStateChanged {
            spec_id: self.id,
            version: self.version.as_u32(),
            from_state: self.state,
            to_state: SpecState::Deleted,
            reason: None,
            changed_by: command.deleted_by,
            changed_at: Utc::now(),
        })])
    }

    pub fn apply_event(mut self, event: &SpecEvent) -> Self {
        match event {
            SpecEvent::Created(_e) => {
                panic!("Cannot apply Created event to existing spec");
            }
            SpecEvent::Updated(e) => {
                self.content = SpecContent::new(e.content.clone()).unwrap();
                if let Some(desc) = &e.description {
                    self.description = Some(desc.clone());
                }
                self.version = Version::new(e.version);
                self.updated_by = e.updated_by.clone();
                self.updated_at = e.updated_at;
            }
            SpecEvent::StateChanged(e) => {
                self.state = e.to_state;
                self.updated_at = e.changed_at;
            }
        }
        self
    }

    pub fn from_events(events: Vec<SpecEvent>) -> Result<Self, DomainError> {
        let mut events_iter = events.into_iter();
        
        let first_event = events_iter.next()
            .ok_or_else(|| DomainError::EventStoreError("No events found".to_string()))?;
        
        let mut spec = match first_event {
            SpecEvent::Created(e) => Self {
                id: e.spec_id,
                name: SpecName::new(e.name)?,
                content: SpecContent::new(e.content)?,
                description: e.description,
                version: Version::initial(),
                state: SpecState::Draft,
                created_at: e.created_at,
                updated_at: e.created_at,
                created_by: e.created_by.clone(),
                updated_by: e.created_by,
            },
            _ => return Err(DomainError::EventStoreError(
                "First event must be Created".to_string()
            )),
        };

        for event in events_iter {
            spec = spec.apply_event(&event);
        }

        Ok(spec)
    }
}