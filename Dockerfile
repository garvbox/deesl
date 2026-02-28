###########################################
# -- Frontend builder stage --
FROM node:22-alpine AS frontend-builder

WORKDIR /app/frontend

COPY frontend/package*.json ./
RUN npm ci

COPY frontend/ ./
RUN npm run build

###########################################
# -- Backend Builder Stage --
FROM rust:bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN mkdir -p src && echo "fn main() {}" > src/main.rs && echo "pub fn dummy() {}" > src/lib.rs

# Build only dependencies - layer caching optimisation
RUN cargo build --release

RUN cargo install diesel_cli --no-default-features --features postgres

COPY . .
RUN touch src/main.rs src/lib.rs

RUN cargo build --release

###########################################
# -- App Stage --
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/deesl /app/deesl
COPY --from=frontend-builder /app/src/pkg /app/src/pkg
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/diesel.toml /app/diesel.toml
COPY --from=builder /usr/local/cargo/bin/diesel /usr/local/bin/diesel

COPY docker-entrypoint.sh /app/
RUN chmod +x /app/docker-entrypoint.sh

ENV PORT=8000
ENV HOST=0.0.0.0

EXPOSE 8000

ENTRYPOINT ["./docker-entrypoint.sh"]
