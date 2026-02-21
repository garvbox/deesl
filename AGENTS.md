# AGENTS.md - Agent Guidelines for deesl

This document provides guidelines for agentic coding agents operating in this repository.

## Project Overview

`deesl` is a Rust web application with a Vue.js frontend:
- **Backend**: Axum with Tower, Diesel (PostgreSQL), Tokio
- **Frontend**: Vue 3 + Vite
- **Database**: Diesel (PostgreSQL) with deadpool for connection pooling
- **Templating**: Askama (for server-rendered pages)
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

## Code Style Guidelines

### General Principles
- Use Rust 2024 edition (as specified in Cargo.toml)
- Prefer explicit over implicit
- Write self-documenting code
- Keep functions small and focused

### Formatting
- Follow standard Rust formatting conventions
- Run `cargo fmt` before committing

### Types
- Use explicit types in public APIs
- Prefer strong typing (newtype patterns) for domain concepts
- Use `Option<T>` for optional values, not sentinel values

### Error Handling
- Use `Result<T, E>` for fallible operations
- Create domain-specific error types for application errors
- Use the `?` operator for error propagation
- Convert errors to HTTP status codes at the handler layer
- Example pattern (from handlers.rs):
  ```rust
  pub async fn list_vehicles(State(pool): State<Pool>) -> Result<Html<String>, (StatusCode, String)> {
      let conn = pool.get().await.map_err(internal_error)?;
      // ...
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
