# Spec Service

A general-purpose service for managing the lifecycle and publishing of YAML specifications using event sourcing.

## Architecture

The service uses event sourcing with SQLite as the event store, providing:
- Complete audit trail of all changes
- Ability to reconstruct any previous state
- Support for spec versioning with states (Draft, Published, Deprecated, Deleted)
- Both REST and gRPC interfaces

### Key Components

1. **Domain Layer** (`src/domain/`)
   - Events: Immutable facts about spec changes
   - Aggregates: Business logic for spec lifecycle
   - Commands: User intentions (Create, Update, Publish, etc.)
   - Value Objects: Type-safe domain primitives

2. **Infrastructure Layer** (`src/infrastructure/`)
   - Event Store: SQLite-based persistence of events
   - Projections: Read models for queries
   - Event Processor: Background worker for updating projections

3. **API Layer** (`src/api/`)
   - REST API using Axum (port 3000)
   - gRPC API using Tonic (port 50051)

## Event Sourcing Benefits

Users interact with intuitive state-based APIs while the system maintains a complete event log:

- **Create Spec**: Generates a `SpecCreated` event
- **Update Content**: Generates a `SpecUpdated` event with new version
- **Publish**: Generates a `StateChanged` event (Draft → Published)
- **Query Current State**: Reconstructs from event stream
- **Query History**: Direct access to all events

## Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs/))
- Protocol Buffers compiler (`protoc`) for gRPC support:
  - macOS: `brew install protobuf`
  - Ubuntu/Debian: `sudo apt-get install protobuf-compiler`
  - Other: See [protobuf installation docs](https://grpc.io/docs/protoc-installation/)

## Getting Started

### Running the Server

```bash
cd spec-server
cargo run
```

This starts both REST (port 3000) and gRPC (port 50051) servers.

### Running Examples

```bash
# Basic event sourcing workflow
cargo run --example basic_workflow

# Projection system demo
cargo run --example projection_example
```

### Testing the APIs

```bash
# Test REST API
./test-api.sh

# Test gRPC API (requires grpcurl)
./test-grpc.sh
```

## Development Status

- ✅ Event sourcing domain model
- ✅ SQLite event store  
- ✅ Core aggregate logic with state machine
- ✅ Projection system with caching
- ✅ REST API with full CRUD + state transitions
- ✅ gRPC API with protobuf definitions
- ✅ Concurrent API servers
- ✅ Architecture documentation (arc42)
- 📋 TUI client (planned)
- 📋 Web client (planned)

## TODO / Next Steps

### Security & Operations
- [ ] Add authentication middleware (JWT/OAuth2)
- [ ] Implement authorization (who can publish/deprecate)
- [ ] Add request rate limiting
- [ ] Implement API versioning strategy
- [ ] Add OpenTelemetry instrumentation
- [ ] Create Dockerfile and docker-compose.yml
- [ ] Add health check endpoints with readiness/liveness
- [ ] Implement graceful shutdown

### Features
- [ ] Add spec validation plugins (custom validators per spec type)
- [ ] Implement spec templates
- [ ] Add bulk operations API
- [ ] Implement spec dependencies/relationships
- [ ] Add webhook notifications for state changes
- [ ] Implement full-text search across specs
- [ ] Add spec diffing API endpoint
- [ ] Support for spec comments/annotations

### Clients
- [ ] Create TUI client using Ratatui
- [ ] Build web UI with WASM (Yew/Leptos)
- [ ] Generate OpenAPI spec for REST API
- [ ] Create SDK libraries (Rust, Python, Go)
- [ ] Build CLI tool for CI/CD integration

### Infrastructure
- [ ] Add PostgreSQL event store option
- [ ] Implement event store partitioning
- [ ] Add Redis caching layer
- [ ] Create Kubernetes manifests
- [ ] Implement backup/restore tools
- [ ] Add event replay capabilities
- [ ] Create data migration tools

### Testing & Quality
- [ ] Add integration test suite
- [ ] Implement property-based tests
- [ ] Add performance benchmarks
- [ ] Create load testing scenarios
- [ ] Add mutation testing
- [ ] Implement contract testing for APIs

### Documentation
- [ ] Create API documentation site
- [ ] Add architecture decision records (ADRs) for new decisions
- [ ] Create deployment guide
- [ ] Write performance tuning guide
- [ ] Add troubleshooting guide
- [ ] Create contributor guidelines

### Advanced Features
- [ ] Multi-tenancy support
- [ ] Spec versioning strategies (semantic versioning)
- [ ] Implement CQRS read model rebuilding
- [ ] Add event sourcing snapshots
- [ ] Create audit report generation
- [ ] Implement spec import/export (from other systems)
- [ ] Add A/B testing for specs
- [ ] Create spec analytics dashboard