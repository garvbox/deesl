
# Run server with reloading and trace logging enabled
develop:
    RUST_LOG=deesl=trace,tower_http=debug cargo watch -x run

# Build Vue frontend for production
build-frontend:
    cd frontend && npm run build

# Run Vue dev server with hot reload
dev-frontend:
    cd frontend && npm run dev
