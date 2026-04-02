# AGENTS.md - Agent Guidelines for deesl

This document provides guidelines for agentic coding agents operating in this repository.

## Project Overview

`deesl` is a full-stack Rust web application:
- **Framework**: Axum with Tower, Diesel (PostgreSQL), Tokio
- **UI Architecture**: Server-Side Rendering (SSR) with Askama templates
- **Interactivity**: HTMX for AJAX-driven partial page updates
- **Database**: Diesel (PostgreSQL) with deadpool for connection pooling
- **Security**: Cookie-based JWT authentication, custom `AuthUser` extractors
- **Testing**: rstest and axum-test

## Build, Lint, and Test Commands

### Development
```bash
# Run server with auto-reload and trace logging
just develop

# Or manually:
RUST_LOG=deesl=trace,tower_http=debug cargo watch -x run
```

### Build
```bash
# Release build
cargo build --release
```

### Linting & Formatting
```bash
# Auto-fix formatting (MUST run before committing)
cargo fmt

# Run clippy
cargo clippy --all --tests --all-features --no-deps
```

### Testing
```bash
# Run all tests (unit + integration)
cargo test
```

### Database
```bash
# Setup database (requires Docker)
docker compose up -d

# Run Diesel migrations
diesel migration run
```

## Development with Auth Bypass

For local testing without Google SSO, set the `DEV_AUTH_EMAIL` environment variable:

```bash
DEV_AUTH_EMAIL=dev@localhost just develop
```

When set, all requests from localhost are treated as authenticated.

## Code Style Guidelines

### SSR & HTMX Patterns
- **Templates**: All HTML is in the `templates/` directory using Askama.
- **Fragments**: Reusable UI parts are in `templates/fragments/`. Use these for HTMX partial swaps.
- **Handlers**: Return `impl IntoResponse` which typically renders a template into `Html<String>`.
- **Forms**: Use `axum::Form` for processing standard HTML form submissions from HTMX.

### Database (Diesel)
- Use `interact` from deadpool-diesel for all database operations.
- Ensure the return type of the `interact` closure is explicitly stated if type inference fails.

### Authentication
- Use `AuthUser` extractor for API/HTMX endpoints (returns 401 on failure).
- Use `AuthUserRedirect` extractor for full-page SSR endpoints (redirects to `/login` on failure).

## Key Patterns

### Version Bumps
When bumping the version in `Cargo.toml`, always run `cargo check` or `cargo build` afterward to verify the change compiles correctly before committing.

### HTMX Partial Update Handler
```rust
pub async fn htmx_list_items(user: AuthUser, State(pool): State<Pool>) -> Result<impl IntoResponse, (StatusCode, String)> {
    // ... logic ...
    let template = ItemsFragment { items };
    Ok(Html(template.render().map_err(internal_error)?))
}
```

### SSR Page Handler
```rust
pub async fn page_handler(AuthUserRedirect(user): AuthUserRedirect) -> impl IntoResponse {
    let template = FullPageTemplate { user_email: user.email };
    Html(template.render().unwrap())
}
```
