use anyhow::Result;
use spec_server::domain::{
    aggregates::Spec,
    commands::{CreateSpec, PublishSpec, UpdateSpec},
    events::{EventMetadata, SpecEvent},
};
use spec_server::infrastructure::event_store::SqliteEventStore;
use uuid::Uuid;

/// Demonstrates the basic workflow of creating, updating, and publishing a spec
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Initialize event store
    let event_store = SqliteEventStore::new("sqlite::memory:").await?;
    event_store.init_schema().await?;

    // Example 1: Create a new spec
    println!("=== Creating a new spec ===");
    
    let create_cmd = CreateSpec {
        name: "regex-validator".to_string(),
        content: r#"
rules:
  - pattern: "^[A-Z][a-z]+$"
    description: "Capitalized word"
    operator: AND
  - pattern: "[0-9]"
    description: "Contains digit"
    operator: NOT
"#.to_string(),
        description: Some("Validates capitalized words without digits".to_string()),
        created_by: "alice@example.com".to_string(),
    };

    let events = Spec::create(create_cmd)?;
    let spec_id = match &events[0] {
        SpecEvent::Created(e) => e.spec_id,
        _ => panic!("Expected Created event"),
    };

    let metadata = EventMetadata {
        correlation_id: Some(Uuid::new_v4()),
        causation_id: None,
        user_agent: Some("example-cli/1.0".to_string()),
        ip_address: Some("127.0.0.1".to_string()),
    };

    event_store.append_events(spec_id, events, metadata.clone()).await?;
    println!("Created spec with ID: {}", spec_id);

    // Load the spec from events
    let events = event_store.get_events(spec_id, None).await?;
    let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())?;
    println!("Loaded spec: {} (version: {})", spec.name, spec.version);

    // Example 2: Update the spec
    println!("\n=== Updating the spec ===");
    
    let update_cmd = UpdateSpec {
        spec_id,
        content: r#"
rules:
  - pattern: "^[A-Z][a-z]+$"
    description: "Capitalized word"
    operator: AND
  - pattern: "[0-9]"
    description: "Contains digit"
    operator: NOT
  - pattern: ".{3,}"
    description: "At least 3 characters"
    operator: AND
"#.to_string(),
        description: Some("Updated: Added minimum length requirement".to_string()),
        updated_by: "bob@example.com".to_string(),
    };

    let update_events = spec.handle_command(IntoSpecCommand::into(update_cmd))?;
    event_store.append_events(spec_id, update_events, metadata.clone()).await?;
    
    // Reload spec
    let events = event_store.get_events(spec_id, None).await?;
    let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())?;
    println!("Updated spec to version: {}", spec.version);

    // Example 3: Publish the spec
    println!("\n=== Publishing the spec ===");
    
    let publish_cmd = PublishSpec {
        spec_id,
        version: Some(spec.version.as_u32()),
        published_by: "admin@example.com".to_string(),
    };

    let publish_events = spec.handle_command(IntoSpecCommand::into(publish_cmd))?;
    event_store.append_events(spec_id, publish_events, metadata).await?;
    
    // Reload spec to see final state
    let events = event_store.get_events(spec_id, None).await?;
    let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())?;
    println!("Spec state: {:?}", spec.state);

    // Example 4: Query all events for audit trail
    println!("\n=== Event History ===");
    let all_events = event_store.get_events(spec_id, None).await?;
    for (i, envelope) in all_events.iter().enumerate() {
        println!("Event {}: {:?}", i + 1, envelope.event);
    }

    Ok(())
}

// Helper trait to convert commands to SpecCommand enum
trait IntoSpecCommand {
    fn into(self) -> spec_server::domain::commands::SpecCommand;
}

impl IntoSpecCommand for UpdateSpec {
    fn into(self) -> spec_server::domain::commands::SpecCommand {
        spec_server::domain::commands::SpecCommand::Update(self)
    }
}

impl IntoSpecCommand for PublishSpec {
    fn into(self) -> spec_server::domain::commands::SpecCommand {
        spec_server::domain::commands::SpecCommand::Publish(self)
    }
}