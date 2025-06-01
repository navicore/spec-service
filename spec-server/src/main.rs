mod api;
mod domain;
mod infrastructure;

use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::api::rest::{create_router, AppState};
use crate::infrastructure::{
    event_processor::EventProcessorManager, event_store::SqliteEventStore,
    projections::ProjectionStore,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tracing::info!("Starting spec-server...");

    // Database URL from environment or default
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:spec_service.db?mode=rwc".to_string());

    tracing::info!("Using database: {}", database_url);

    // Initialize stores
    let event_store = Arc::new(SqliteEventStore::new(&database_url).await?);
    let projection_store = Arc::new(ProjectionStore::new(&database_url, true).await?);

    // Initialize schemas
    tracing::info!("Initializing database schemas...");
    event_store.init_schema().await?;
    projection_store.init_schema().await?;

    // Start event processor
    tracing::info!("Starting event processor...");
    let manager = EventProcessorManager::new(event_store.clone(), projection_store.clone());
    let (_processor_handle, _shutdown_tx) = manager.start_background();

    // Create app state
    let app_state = AppState {
        event_store: event_store.clone(),
        projection_store: projection_store.clone(),
    };

    // Create REST router
    let app = create_router(app_state).layer(TraceLayer::new_for_http());

    // Start REST server
    let rest_addr = std::env::var("REST_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    tracing::info!("REST API listening on {}", rest_addr);

    let listener = tokio::net::TcpListener::bind(&rest_addr).await?;
    let rest_server = axum::serve(listener, app);

    // Start gRPC server
    let grpc_addr = std::env::var("GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
        .parse()?;

    tracing::info!("gRPC API listening on {}", grpc_addr);

    let grpc_service =
        api::grpc::SpecServiceImpl::new(event_store.clone(), projection_store.clone());

    let grpc_server = tonic::transport::Server::builder()
        .add_service(grpc_service.into_service())
        .serve(grpc_addr);

    // Run both servers concurrently
    tokio::select! {
        res = rest_server => {
            if let Err(e) = res {
                tracing::error!("REST server error: {}", e);
            }
        }
        res = grpc_server => {
            if let Err(e) = res {
                tracing::error!("gRPC server error: {}", e);
            }
        }
    }

    Ok(())
}
