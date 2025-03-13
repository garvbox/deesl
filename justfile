
# Run server with reloading and trace logging enabled
develop:
    RUST_LOG=deesl=trace,tower_http=debug cargo watch -x run
