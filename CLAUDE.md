# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Commands

### Build
```bash
cargo build --release           # Full workspace build
cargo build -p hsb-server        # Build specific crate
```

### Run
```bash
# Generate default config
./target/release/hsb-server init --output config/hsb.toml

# Check configuration
./target/release/hsb-server check --config config/hsb.toml

# Start server
./target/release/hsb-server start --config config/hsb.toml

# Quick local dev (uses start.sh with Docker for dependencies)
./start.sh
```

### Docker
```bash
# Full build from source
docker build -t hsb-server:latest .

# Prebuilt mode (faster, requires existing binary)
docker build --build-arg BUILDMODE=prebuilt -t hsb-server:latest .

# Start container
./docker-start.sh
```

### Test
```bash
cargo test --workspace            # Run all tests
cargo test -p hsb-core            # Test specific crate
```

### Lint
```bash
cargo clippy --workspace
cargo fmt --check
```

## Architecture Overview

Nexus HSB is a medical information integration platform with a six-layer architecture:

1. **hsb-admin** (Management): Admin API, configuration management, monitoring, log visualization
2. **hsb-core/persistence** (Audit): Message tracking, audit logs, compliance records, metrics
3. **hsb-core/reliability** (Reliability): Message queues, retry mechanisms, dead letter queues, circuit breakers
4. **hsb-core/engine** (Message Engine): Routing engine, message transformation, processing pipeline, component registry
5. **hsb-adapter** (Protocol Adapters): HL7 v2.x/v3, FHIR R5, DICOM, SOAP, Custom protocols
6. **hsb-transport** (Transports): HTTP/HTTPS, Webhook, TCP/MLLP, gRPC, RabbitMQ, Kafka, NATS

### Crate Structure
```
hsb-server/       # Main binary entry point (HTTP/TCP/gRPC listeners)
hsb-admin/        # Admin REST API (state management, routes, endpoints, workflows)
hsb-core/         # Core models, routing engine, persistence, reliability, cluster
hsb-adapter/      # Protocol adapters (HL7, FHIR, DICOM, SOAP)
hsb-transport/    # Transport layers (HTTP, TCP, MQ, gRPC)
hsb-common/       # Shared types, error handling, utilities
hsb-mock-endpoints/ # Mock services for testing
```

### Key Architectural Patterns

**Protocol Adapter Pattern**: All inbound messages are parsed through protocol-specific adapters defined in `hsb-adapter/`. Each protocol (HL7V2, FHIR, DICOM, SOAP) implements `ProtocolAdapter` trait to convert raw payloads into canonical `Message` objects.

**Transport Abstraction**: Outbound delivery uses `hsb-transport/` with `Transport` trait. Transports are protocol-independent (HTTP, TCP, MQ, gRPC) and are selected based on endpoint configuration.

**Routing Engine**: Messages are routed based on `Route` rules defined in `hsb-core/src/route.rs`. Routes match against source system, message type, and custom conditions, then dispatch to target endpoints with configurable delivery modes (`EXACTLY_ONCE`, `AT_LEAST_ONCE`, `AT_MOST_ONCE`).

**Processing Pipeline**: In `hsb-core/src/engine/pipeline.rs`, messages flow through configurable processing stages (validation, transformation, enrichment) before routing.

**Workflow Engine**: `hsb-core/src/workflow.rs` provides orchestration capabilities for multi-step business processes with compensation policies.

**Persistence Dual Mode**: Messages are stored in both Redb (embedded KV) and PostgreSQL. Redb provides fast local caching; PostgreSQL enables distributed queries and audit compliance.

**Reliability Layer**: Circuit breakers, retry with exponential backoff, and dead letter queue in `hsb-core/src/reliability/` ensure message delivery resilience.

### Message Flow

```
Inbound (HTTP/TCP/MQ)
  ↓
MessageIngressRuntime
  ↓
ProtocolAdapter (HL7/FHIR/DICOM/JSON/etc.)
  ↓
Canonical Message (hsb-core/src/message.rs)
  ↓
ProcessingPipeline (validation/transformation)
  ↓
Router (route matching)
  ↓
Dispatcher (multi-target delivery)
  ↓
Transport (HTTP/TCP/MQ/gRPC)
  ↓
Outbound endpoints
```

## Configuration

Configuration is TOML-based in `config/hsb.toml`. Key sections:
- `[http]`: HTTP ports (8080 main, 8081 admin)
- `[tcp]`: TCP/MLLP port (2575) for HL7v2
- `[grpc]`: gRPC port (10051) for SSO integration
- `[database]`: PostgreSQL connection for persistence
- `[persistence]`: Redb path + PostgreSQL toggle
- `[mq]` / `[kafka]` / `[nats]`: Message queue connections
- `[sso]`: rust-sso integration for authentication

## Important Notes

- **ULID for IDs**: Uses `ulid` crate instead of UUID for message IDs
- **Edition 2024**: Workspace uses Rust 2024 edition
- **MLLP for HL7v2**: TCP port 2575 uses MLLP framing (0x0B start, 0x1C 0x0D end)
- **Protocol Auto-Detection**: HTTP inbound auto-detects protocol based on Content-Type and body patterns
- **Admin API**: Available on both port 8081 and via `/api/v1/` prefix on main port 8080
- **Docker Multi-Stage**: Dockerfile supports both `full` (source build) and `prebuilt` (copy binary) modes
- **Dependencies**: Requires PostgreSQL 14+ for persistence, RabbitMQ/NATS for queue transport