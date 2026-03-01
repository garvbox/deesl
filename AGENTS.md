# AGENTS.md - Agent Guidelines for deesl

This document provides guidelines for agentic coding agents operating in this repository.

## Project Overview

`deesl` is a Rust web application with a Vue.js frontend:
- **Backend**: Axum with Tower, Diesel (PostgreSQL), Tokio
- **Frontend**: Vue 3 + Vite
- **Database**: Diesel (PostgreSQL) with deadpool for connection pooling
- **Templating**: Askama (for OpenAPI documentation)
- **Async Runtime**: Tokio
- **Testing**: rstest

## Build, Lint, and Test Commands

### Development
```bash
# Run backend server with auto-reload and trace logging
just develop

# Or manually:
RUST_LOG=deesl=trace,tower_http=debug cargo watch -x run

# Run Vue dev server (port 5173, proxies /api to backend)
just dev-frontend

# Build Vue frontend for production (outputs to src/pkg/)
just build-frontend
```

### Build
```bash
# Release build
cargo build --release

# Debug build
cargo build
```

### Linting & Formatting

**IMPORTANT: Always run `cargo fmt` after modifying Rust files before committing.**

```bash
# Run all lints (via pre-commit)
pre-commit run --all-files

# Check formatting (cargo fmt)
cargo fmt -- --check

# Auto-fix formatting
cargo fmt

# Run clippy
cargo clippy --all --tests --all-features --no-deps
```

### Testing
```bash
# Run all tests
cargo test

# Run a single test by name
cargo test test_name_here

# Run tests matching a pattern
cargo test pattern

# Run tests with output
cargo test -- --nocapture

# Run doc tests
cargo test --doc

# Run tests with specific features
cargo test --all-features
```

### Database
```bash
# Setup database (requires Docker)
docker compose up -d

# Run Diesel migrations
diesel migration run

# Revert last migration
diesel migration redo

# Generate schema from database
diesel print-schema > src/schema.rs
```

### Development with Auth Bypass

For local testing without Google SSO, set the `DEV_AUTH_EMAIL` environment variable:

```bash
# Run backend with auth bypass enabled
DEV_AUTH_EMAIL=dev@localhost cargo run

# Or with the just command
DEV_AUTH_EMAIL=dev@localhost just develop
```

When `DEV_AUTH_EMAIL` is set, all requests from localhost are treated as authenticated as user_id=1 with the specified email.

**Production Safety:** The auth bypass has multiple layers of protection:
1. **Compile-time:** Only works in debug builds (`cargo run`), not release builds
2. **Network:** Only accepts requests from localhost (127.0.0.1, ::1, localhost)

This is useful for:
- Testing UI flows with agent-browser
- API development and debugging
- Integration testing without OAuth setup

**⚠️ Never use this in production or CI environments.**

## Code Style Guidelines

### General Principles
- Use Rust 2024 edition (as specified in Cargo.toml)
- Prefer explicit over implicit
- Write self-documenting code
- Keep functions small and focused

### Formatting
- Follow standard Rust formatting conventions
- **MUST run `cargo fmt` after any Rust file modifications**

### Types
- Use explicit types in public APIs
- Prefer strong typing (newtype patterns) for domain concepts
- Use `Option<T>` for optional values, not sentinel values

### Error Handling
- Use `Result<T, E>` for fallible operations
- Create domain-specific error types for application errors
- Use the `?` operator for error propagation
- Convert errors to HTTP status codes at the handler layer
- Example pattern:
  ```rust
  pub async fn create_vehicle(
      State(pool): State<Pool>,
      Json(payload): Json<CreateVehicleRequest>,
  ) -> Result<impl IntoResponse, (StatusCode, String)> {
      let conn = pool.get().await.map_err(internal_error)?;
      // ... business logic
      Ok((StatusCode::CREATED, Json(VehicleResponse::from(vehicle))))
  }

  pub fn internal_error<E>(err: E) -> (StatusCode, String)
  where
      E: std::error::Error,
  {
      (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
  }
  ```

### Async Code
- Use `async fn` for async handlers
- Use Tokio as the async runtime
- Be explicit about `.await`
- Use `interact` from deadpool-diesel for database operations in async context

### Database (Diesel)
- Schema is auto-generated; do **not** manually edit `src/schema.rs`
- Use `diesel print-schema` to regenerate after migration changes
- Follow Diesel conventions for model types:
  ```rust
  #[derive(Queryable, Selectable, serde::Serialize)]
  #[diesel(table_name = crate::schema::vehicles)]
  #[diesel(check_for_backend(diesel::pg::Pg))]
  pub struct Vehicle { ... }
  
  #[derive(Insertable, serde::Deserialize)]
  #[diesel(table_name = crate::schema::vehicles)]
  pub struct NewVehicle { ... }
  ```

### Testing
- Use `rstest` for parameterized tests
- Place tests in the same file (inline `#[cfg(test)]` modules) or in `tests/` directory
- Use descriptive test names: `should_return_vehicles_when_database_has_data()`
- Include doc tests for public APIs

### Frontend (Vue.js)
- Use Vue 3 Composition API with `<script setup>` syntax
- Place reusable logic in `composables/` directory (e.g., `useAuth.js`)
- Place API calls in `services/` directory
- Use `ref()` for reactive state, `computed()` for derived state
- Components are in `components/` directory
- Styles are scoped within each `.vue` file's `<style>` block

## Common Patterns

### Handler Pattern
```rust
pub async fn handler_name(
    State(pool): State<Pool>,
    // Other extractors...
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = pool.get().await.map_err(internal_error)?;
    // Business logic...
    Ok(response)
}
```

### Configuration Pattern
```rust
#[derive(Debug)]
pub struct Config {
    field: Type,
}

impl Config {
    fn new() -> Self {
        Self {
            field: env::var("KEY").unwrap_or_default(),
        }
    }
}
```

## Dependencies

### Backend (see `Cargo.toml`)
- `axum` - HTTP framework
- `diesel` / `deadpool-diesel` - Database ORM and connection pool
- `tokio` - Async runtime
- `askama` - Template engine
- `serde` - Serialization
- `tracing` / `tracing-subscriber` - Logging
- `rstest` - Testing framework
- `tower-http` - CORS, static file serving, tracing

### Frontend (see `frontend/package.json`)
- `vue` - UI framework
- `vite` - Build tool and dev server
