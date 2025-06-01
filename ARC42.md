# Spec Service Architecture Documentation (arc42)

**About arc42**: This document follows the arc42 template for architecture documentation, providing a comprehensive view of the system architecture that all stakeholders can reference.

## 1. Introduction and Goals

### 1.1 Requirements Overview

The Spec Service is a general-purpose service for managing the lifecycle and publishing of YAML specifications. These specifications are small (typically <2KB) YAML documents that define various business rules and configurations, such as:
- Regex validation rules with logical operators
- Approval workflow definitions
- Other structured configuration data

### 1.2 Quality Goals

| Priority | Quality Goal | Description |
|----------|-------------|-------------|
| 1 | Auditability | Complete history of all changes with who, what, when, and why |
| 2 | Simplicity | Easy to understand state model (Draft → Published → Deprecated) |
| 3 | Reliability | Lossless event storage, ability to reconstruct any previous state |
| 4 | Performance | Fast queries for current state and version history |
| 5 | Extensibility | Easy to add new spec types and state transitions |

### 1.3 Stakeholders

| Role | Concerns |
|------|----------|
| Developers | Clear API, versioning, ability to test with different spec versions |
| Operations | Audit trail, rollback capability, monitoring |
| Compliance | Complete history, access control, change justification |
| End Users | Simple state model, clear version progression |

## 2. Architecture Constraints

- Specs are always YAML format, single file, <2KB
- Must support both REST and gRPC interfaces
- SQLite for initial implementation (embedded, simple deployment)
- Rust for type safety and performance
- Event sourcing for audit requirements

## 3. System Scope and Context

### 3.1 Business Context

See `docs/architecture-diagrams.puml` - System Context Diagram

The Spec Service provides versioned specification management with full audit trail to various stakeholders:
- **Developers**: Create and update specifications, test with specific versions
- **Operations**: Manage spec lifecycle (publish, deprecate)
- **Compliance**: Review complete change history
- **Service Consumers**: Fetch current specifications via REST/gRPC APIs

## 4. Solution Strategy

### 4.1 Event Sourcing Approach

We use event sourcing to capture all changes as immutable events:
- **Events**: SpecCreated, SpecUpdated, SpecStateChanged
- **Aggregates**: Spec (enforces business rules)
- **Projections**: Read-optimized views of current state

### 4.2 State Management

Simple, intuitive states that hide event sourcing complexity:
- Draft → Published → Deprecated → Deleted
- Version increments on content updates
- State transitions via explicit commands

## 5. Building Block View

### 5.1 Level 1: System Components

See `docs/architecture-diagrams.puml` - Building Block Diagram

The system is organized in layers following Domain-Driven Design principles:

- **API Layer**: External interfaces (REST, gRPC)
- **Application Layer**: Command/Query handlers, event bus
- **Domain Layer**: Core business logic, aggregates, events
- **Infrastructure Layer**: Event store, projections, repositories

Key architectural decisions:
- Command Query Responsibility Segregation (CQRS) 
- Event sourcing for write model
- Projections for optimized read models

## 6. Runtime View

### 6.1 Create and Publish Spec

See `docs/architecture-diagrams.puml` - Runtime Sequence Diagram

The sequence diagram shows the complete flow for:
1. **Creating a spec**: Validation, event generation, persistence, and projection update
2. **Updating content**: Loading from events, version increment, new event
3. **Publishing**: State transition validation, state change event

Key patterns:
- Commands are validated by aggregates before generating events
- Events are persisted before updating projections
- Read models are eventually consistent

## 7. Deployment View

See `docs/architecture-diagrams.puml` - Deployment Diagram

### 7.1 Development
- Single Rust binary with embedded SQLite file
- Zero infrastructure requirements

### 7.2 Production (Initial)
- Docker container with SQLite volume mount
- Reverse proxy for SSL termination
- Simple, single-node deployment

### 7.3 Production (Scaled)
- Multiple spec-server instances
- PostgreSQL for event store (maintaining event sourcing)
- Redis for query caching
- Load balancer for distribution

## 8. Cross-cutting Concepts

### 8.1 Event Structure

All events contain:
- Aggregate ID (Spec ID)
- Event type and payload
- Metadata (user, timestamp, correlation ID)
- Sequence number for ordering

### 8.2 Error Handling

- Domain errors (InvalidStateTransition, VersionMismatch)
- Infrastructure errors (database, network)
- Validation errors (invalid YAML, name constraints)

## 9. Architecture Decisions

### 9.1 ADR-001: Event Sourcing with SQLite Instead of Git

**Date**: 2024-05-31

**Status**: Accepted

**Context**: 
We need to store specification versions with complete audit trail. Git was considered as it's proven for version control and some teams have used it as a database for CMS-like systems.

**Decision**: 
Use event sourcing with SQLite instead of Git as the persistence layer.

**Consequences**:

Positive:
- **Domain-specific modeling**: Events match our business language (SpecPublished vs git commit)
- **Rich queries**: SQL enables "show all published specs" with simple indexed queries
- **Structured metadata**: First-class fields for who, when, why, correlation IDs
- **State management**: Business states (Draft/Published) are explicit, not convention-based
- **Performance**: Indexed queries vs filesystem scanning
- **Transactional integrity**: ACID guarantees for complex operations

Negative:
- Must implement our own versioning (but it's domain-specific)
- No built-in diff visualization (but can compute on-demand)
- Need backup strategy (but SQLite is simple to backup)

**Alternatives Considered**:
- **Git-based**: Would require complex conventions for states, poor query performance, impedance mismatch with REST/gRPC APIs
- **Traditional CRUD**: Would lose audit trail and event history

### 9.2 ADR-002: Full Snapshot Events Instead of Diff/Patch

**Date**: 2024-05-31

**Status**: Accepted

**Context**: 
In event sourcing, we can store either complete content in each event or just the changes (diffs).

**Decision**: 
Store complete YAML content in each SpecUpdated event rather than diffs.

**Consequences**:

Positive:
- **Simplicity**: No patch/merge complexity
- **Self-contained events**: Each event has full context
- **Flexible diffing**: Can compute diffs between any versions on-demand
- **Reliability**: No risk of corruption from failed patch application
- **Fast access**: Show any version with single event lookup

Negative:
- More storage used (negligible for <2KB files)
- Changes not immediately visible in events (can diff on-demand)

**Rationale**: 
For small specs (<2KB), storage overhead is minimal. Simplicity and reliability outweigh the minor storage cost.

### 9.3 ADR-003: Purpose-Built Instead of Repurposing MLM Tools

**Date**: 2024-05-31

**Status**: Accepted

**Context**: 
Model Lifecycle Management (MLM) tools from 2016-2020 (MLflow, Kubeflow, Neptune.ai) handle versioning and governance. Could we repurpose them for YAML spec management?

**Decision**: 
Build a purpose-specific service rather than repurposing MLM tools.

**Consequences**:

Positive:
- **Right-sized**: No overhead from unused ML features (experiment tracking, metrics, GPU monitoring)
- **Domain language**: "Published spec" not "Production model"
- **Cost effective**: No infrastructure overhead or SaaS fees
- **Simple operations**: Single SQLite file vs tracking servers, artifact stores, and databases
- **Developer experience**: Clear, focused API without ML concepts

Negative:
- Must implement features that MLM tools provide (but only the ones we need)
- No existing ecosystem (but MLM ecosystem doesn't fit our needs)

**Alternatives Considered**:
- **MLflow**: Requires tracking server, database, artifact store - massive overhead for 2KB files
- **DVC**: Git-based, similar limitations to direct Git usage
- **Neptune/W&B**: Expensive SaaS with features we don't need

**Rationale**: 
MLM tools assume large model files, experiment tracking, and compute resource management. Our simple YAML specs need domain-specific state management, not generic "model lifecycle" features. The infrastructure and cost overhead cannot be justified.

## 10. Quality Requirements

### 10.1 Performance
- Query current state: <10ms
- Retrieve version history: <50ms
- Apply state change: <100ms

### 10.2 Reliability
- Zero data loss (event sourcing guarantee)
- Ability to rebuild from events
- SQLite durability with WAL mode

### 10.3 Maintainability
- Clear domain boundaries
- Comprehensive examples/tests
- Simple deployment model

## 11. Risks and Technical Debt

### 11.1 Risks
- SQLite scalability limits (mitigated by PostgreSQL migration path)
- Event schema evolution (mitigated by versioned events)

### 11.2 Technical Debt
- Projections not yet implemented
- REST/gRPC APIs not yet complete
- No authentication/authorization layer yet

## 12. Glossary

| Term | Definition |
|------|------------|
| Spec | A YAML document defining business rules or configuration |
| Event | An immutable fact about something that happened |
| Aggregate | Domain object that enforces business rules |
| Projection | Read-optimized view built from events |
| Event Sourcing | Storing state as a sequence of events |

---

This architecture documentation is a living document and should be updated as the system evolves and new decisions are made.