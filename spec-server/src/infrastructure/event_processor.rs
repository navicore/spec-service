use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

use super::event_store::SqliteEventStore;
use super::projections::ProjectionStore;
use crate::domain::errors::DomainError;

/// Processes events from the event store and updates projections
pub struct EventProcessor {
    event_store: Arc<SqliteEventStore>,
    projection_store: Arc<ProjectionStore>,
    shutdown_rx: mpsc::Receiver<()>,
}

impl EventProcessor {
    pub fn new(
        event_store: Arc<SqliteEventStore>,
        projection_store: Arc<ProjectionStore>,
        shutdown_rx: mpsc::Receiver<()>,
    ) -> Self {
        Self {
            event_store,
            projection_store,
            shutdown_rx,
        }
    }

    /// Start processing events from the given position
    pub async fn start(mut self, from_position: i64) -> Result<()> {
        info!("Starting event processor from position {}", from_position);

        let mut current_position = from_position;
        let batch_size = 100;
        let poll_interval = Duration::from_millis(100);

        loop {
            // Check for shutdown signal
            if self.shutdown_rx.try_recv().is_ok() {
                info!("Event processor received shutdown signal");
                break;
            }

            // Fetch next batch of events
            match self.process_batch(current_position, batch_size).await {
                Ok(processed_count) => {
                    if processed_count > 0 {
                        current_position += i64::try_from(processed_count).unwrap_or(i64::MAX);
                        info!(
                            "Processed {} events, new position: {}",
                            processed_count, current_position
                        );
                    } else {
                        // No new events, wait before polling again
                        sleep(poll_interval).await;
                    }
                }
                Err(e) => {
                    error!("Error processing events: {}", e);
                    // Wait before retrying
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }

        info!("Event processor stopped");
        Ok(())
    }

    async fn process_batch(&self, from_position: i64, limit: i64) -> Result<usize, DomainError> {
        let events = self
            .event_store
            .get_all_events(from_position, limit)
            .await?;

        let mut processed_count = 0;

        for (aggregate_id, envelope) in events {
            match self
                .projection_store
                .apply_event(aggregate_id, &envelope.event)
                .await
            {
                Ok(()) => {
                    processed_count += 1;
                }
                Err(e) => {
                    warn!(
                        "Failed to apply event {} to projections: {}",
                        envelope.event_id, e
                    );
                    // Continue processing other events
                }
            }
        }

        Ok(processed_count)
    }
}

/// Manages the lifecycle of the event processor
pub struct EventProcessorManager {
    event_store: Arc<SqliteEventStore>,
    projection_store: Arc<ProjectionStore>,
}

impl EventProcessorManager {
    pub fn new(event_store: Arc<SqliteEventStore>, projection_store: Arc<ProjectionStore>) -> Self {
        Self {
            event_store,
            projection_store,
        }
    }

    /// Start the event processor in a background task
    pub fn start_background(self) -> (tokio::task::JoinHandle<Result<()>>, mpsc::Sender<()>) {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        let processor = EventProcessor::new(self.event_store, self.projection_store, shutdown_rx);

        // TODO: Load last processed position from checkpoint table
        let from_position = 0;

        let handle = tokio::spawn(async move { processor.start(from_position).await });

        (handle, shutdown_tx)
    }

    /// Rebuild all projections from scratch
    #[allow(dead_code)]
    pub async fn rebuild_projections(&self) -> Result<(), DomainError> {
        info!("Rebuilding all projections from events");

        // Clear existing projections
        sqlx::query("DELETE FROM spec_projections")
            .execute(&self.projection_store.pool)
            .await
            .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        sqlx::query("DELETE FROM spec_version_history")
            .execute(&self.projection_store.pool)
            .await
            .map_err(|e| DomainError::ProjectionError(e.to_string()))?;

        // Process all events from the beginning
        let mut position = 0;
        let batch_size = 1000;

        loop {
            let events = self
                .event_store
                .get_all_events(position, batch_size)
                .await?;

            if events.is_empty() {
                break;
            }

            for (aggregate_id, envelope) in &events {
                self.projection_store
                    .apply_event(*aggregate_id, &envelope.event)
                    .await?;
            }

            position += i64::try_from(events.len()).unwrap_or(i64::MAX);
            info!("Rebuilt {} projections", position);
        }

        info!("Projection rebuild complete");
        Ok(())
    }
}
