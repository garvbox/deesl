# Run server with reloading and trace logging enabled
develop:
    RUST_LOG=deesl=trace,tower_http=debug cargo watch -x "run --features dev"

# Format all Rust code
fmt:
    cargo fmt

# Run all lints (clippy and formatting check)
lint:
    cargo fmt -- --check
    cargo clippy --all-targets --all-features -- -D warnings
