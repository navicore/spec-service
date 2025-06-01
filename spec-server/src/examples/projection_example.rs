use anyhow::Result;
use spec_server::domain::{
    aggregates::Spec,
    commands::{CreateSpec, DeprecateSpec, PublishSpec, UpdateSpec},
    events::{EventMetadata, SpecEvent, SpecState},
};
use spec_server::infrastructure::{
    event_processor::EventProcessorManager, event_store::SqliteEventStore,
    projections::ProjectionStore,
};
use std::sync::Arc;
use uuid::Uuid;

/// Demonstrates the projection system providing fast queries
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Initialize stores
    let db_url = "sqlite::memory:";
    let event_store = Arc::new(SqliteEventStore::new(db_url).await?);
    let projection_store = Arc::new(ProjectionStore::new(db_url, true).await?);

    // Initialize schemas
    event_store.init_schema().await?;
    projection_store.init_schema().await?;

    // Start event processor in background
    let manager = EventProcessorManager::new(event_store.clone(), projection_store.clone());
    let (_handle, _shutdown) = manager.start_background();

    // Give processor time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("=== Creating multiple specs ===");

    // Create several specs
    let specs = vec![
        ("auth-rules", "Authentication rules", "alice@example.com"),
        (
            "validation-rules",
            "Input validation patterns",
            "bob@example.com",
        ),
        (
            "approval-workflow",
            "Approval chain configuration",
            "carol@example.com",
        ),
    ];

    let mut spec_ids = Vec::new();

    for (name, desc, user) in specs {
        let events = Spec::create(CreateSpec {
            name: name.to_string(),
            content: format!("# {} spec\nversion: 1.0\nrules: []", name),
            description: Some(desc.to_string()),
            created_by: user.to_string(),
        })?;

        let spec_id = match &events[0] {
            SpecEvent::Created(e) => e.spec_id,
            _ => panic!("Expected Created event"),
        };

        spec_ids.push((spec_id, name));

        event_store
            .append_events(
                spec_id,
                events,
                EventMetadata {
                    correlation_id: Some(Uuid::new_v4()),
                    causation_id: None,
                    user_agent: None,
                    ip_address: None,
                },
            )
            .await?;
    }

    // Wait for projections to update
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    println!("\n=== Querying all draft specs ===");

    let draft_specs = projection_store
        .list_by_state(Some(SpecState::Draft), 10, 0)
        .await?;
    for spec in &draft_specs {
        println!(
            "- {} (v{}) - {}",
            spec.name,
            spec.latest_version,
            spec.description.as_deref().unwrap_or("")
        );
    }
    println!("Total draft specs: {}", draft_specs.len());

    // Update and publish some specs
    println!("\n=== Publishing specs ===");

    for (spec_id, name) in &spec_ids[..2] {
        // Load current state
        let events = event_store.get_events(*spec_id, None).await?;
        let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())?;

        // Publish
        let publish_events = spec.handle_command(IntoSpecCommand::into(PublishSpec {
            spec_id: *spec_id,
            version: Some(1),
            published_by: "admin@example.com".to_string(),
        }))?;

        event_store
            .append_events(*spec_id, publish_events, EventMetadata::default())
            .await?;

        println!("Published: {}", name);
    }

    // Wait for projections
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    println!("\n=== Querying published specs ===");

    let published_specs = projection_store
        .list_by_state(Some(SpecState::Published), 10, 0)
        .await?;
    for spec in &published_specs {
        println!("- {} (v{}) - Published", spec.name, spec.latest_version);
    }

    // Query by name
    println!("\n=== Query by name ===");

    if let Some(spec) = projection_store.get_by_name("auth-rules").await? {
        println!("Found spec 'auth-rules':");
        println!("  ID: {}", spec.id);
        println!("  Version: {}", spec.version);
        println!("  State: {:?}", spec.state);
        println!("  Updated by: {}", spec.updated_by);
    }

    // Update a spec and check version history
    println!("\n=== Version history example ===");

    let (spec_id, name) = &spec_ids[0];
    let events = event_store.get_events(*spec_id, None).await?;
    let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())?;

    // Update content
    let update_events = spec.handle_command(IntoSpecCommand::into(UpdateSpec {
        spec_id: *spec_id,
        content: format!(
            "# {} spec\nversion: 2.0\nrules:\n  - pattern: .*\n    action: allow",
            name
        ),
        description: Some("Updated with new rules".to_string()),
        updated_by: "alice@example.com".to_string(),
    }))?;

    event_store
        .append_events(*spec_id, update_events, EventMetadata::default())
        .await?;

    // Wait for projection
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Query current version
    if let Some(current) = projection_store.get_by_id(*spec_id).await? {
        println!(
            "\nCurrent version of '{}': v{}",
            current.name, current.version
        );
    }

    // Query specific version
    if let Some((content, _desc)) = projection_store.get_version(*spec_id, 1).await? {
        println!(
            "\nVersion 1 content preview: {}",
            &content[..50.min(content.len())]
        );
    }

    // Demonstrate deprecation
    println!("\n=== Deprecating a spec ===");

    let events = event_store.get_events(*spec_id, None).await?;
    let spec = Spec::from_events(events.into_iter().map(|e| e.event).collect())?;

    let deprecate_events = spec.handle_command(IntoSpecCommand::into(DeprecateSpec {
        spec_id: *spec_id,
        reason: "Replaced by auth-rules-v2".to_string(),
        deprecated_by: "admin@example.com".to_string(),
    }))?;

    event_store
        .append_events(*spec_id, deprecate_events, EventMetadata::default())
        .await?;

    // Wait and query
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let all_non_deleted = projection_store.list_by_state(None, 10, 0).await?;
    println!("\nAll non-deleted specs:");
    for spec in all_non_deleted {
        println!(
            "- {} (v{}) - State: {:?}",
            spec.name, spec.latest_version, spec.state
        );
    }

    Ok(())
}

// Helper trait for command conversion
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

impl IntoSpecCommand for DeprecateSpec {
    fn into(self) -> spec_server::domain::commands::SpecCommand {
        spec_server::domain::commands::SpecCommand::Deprecate(self)
    }
}
