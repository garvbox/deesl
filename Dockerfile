FROM rust:bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

RUN cargo install diesel_cli --no-default-features --features postgres

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY frontend/Cargo.toml ./frontend/

RUN mkdir -p src && echo "fn main() {}" > src/main.rs && \
    mkdir -p frontend/src && echo "" > frontend/src/lib.rs

RUN cargo build --release

COPY . .

RUN cd frontend && wasm-pack build --target web --out-dir ../src/pkg --release

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install diesel_cli --no-default-features --features postgres || true

WORKDIR /app

COPY --from=builder /app/target/release/deesl /app/deesl
COPY --from=builder /app/src/pkg /app/src/pkg
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/diesel.toml /app/diesel.toml
COPY --from=builder /usr/local/cargo/bin/diesel /usr/local/bin/diesel

COPY docker-entrypoint.sh /app/
RUN chmod +x /app/docker-entrypoint.sh

ENV PORT=8000
ENV HOST=0.0.0.0

EXPOSE 8000

ENTRYPOINT ["./docker-entrypoint.sh"]
