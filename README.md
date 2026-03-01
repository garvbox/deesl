# deesl

[![Coverage](https://codecov.io/gh/garvbox/deesl/graph/badge.svg)](https://codecov.io/gh/garvbox/deesl)

A full-stack Rust web application for tracking vehicle fuel consumption, built with Axum, Askama, and HTMX.

## Architecture

- **Backend**: Axum (Rust)
- **Database**: PostgreSQL with Diesel ORM
- **Templating**: Askama (Type-safe compiled templates)
- **Frontend Interactivity**: HTMX (Hypermedia-driven interactivity)
- **Security**: Cookie-based JWT authentication with Google OAuth2 support

## Development

### Prerequisites
- Rust 1.82+ (2024 edition)
- Docker (for database)
- [Diesel CLI](https://diesel.rs/guides/getting-started)

### Getting Started
```bash
# Start database
docker compose up -d

# Run migrations
diesel migration run

# Run application with auto-reload
just develop
```

### Testing
```bash
# Run all tests (unit + integration)
cargo test
```
