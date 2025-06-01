use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum SpecCommand {
    Create(CreateSpec),
    Update(UpdateSpec),
    Publish(PublishSpec),
    Deprecate(DeprecateSpec),
    Delete(DeleteSpec),
}

#[derive(Debug, Clone)]
pub struct CreateSpec {
    pub name: String,
    pub content: String,
    pub description: Option<String>,
    pub created_by: String,
}

#[derive(Debug, Clone)]
pub struct UpdateSpec {
    pub spec_id: Uuid,
    pub content: String,
    pub description: Option<String>,
    pub updated_by: String,
}

#[derive(Debug, Clone)]
pub struct PublishSpec {
    pub spec_id: Uuid,
    pub version: Option<u32>,
    pub published_by: String,
}

#[derive(Debug, Clone)]
pub struct DeprecateSpec {
    pub spec_id: Uuid,
    pub reason: String,
    pub deprecated_by: String,
}

#[derive(Debug, Clone)]
pub struct DeleteSpec {
    pub spec_id: Uuid,
    pub deleted_by: String,
}

pub struct CommandContext {
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
}