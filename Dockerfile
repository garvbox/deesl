###########################################
# -- Chef Base --
FROM public.ecr.aws/docker/library/rust:bookworm AS chef

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --version 0.1.78 --locked

# Install diesel_cli before copying source so this layer stays cached across releases
RUN cargo install diesel_cli --version 2.3.14 --no-default-features --features postgres --locked

WORKDIR /app

###########################################
# -- Dependency Planner --
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

###########################################
# -- Backend Builder --
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

###########################################
# -- App Stage --
FROM public.ecr.aws/docker/library/debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/deesl /app/deesl
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/diesel.toml /app/diesel.toml
COPY --from=builder /usr/local/cargo/bin/diesel /usr/local/bin/diesel

COPY docker-entrypoint.sh /app/
RUN chmod +x /app/docker-entrypoint.sh

ENV PORT=8000
ENV HOST=0.0.0.0

EXPOSE 8000

ENTRYPOINT ["./docker-entrypoint.sh"]
