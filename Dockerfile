FROM rust:1.97-bookworm AS build

RUN apt-get update \
    && apt-get install --no-install-recommends --yes pkg-config libdbus-1-dev libwayland-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates libdbus-1-3 libwayland-client0 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /workspace/target/release/arqen /usr/local/bin/arqen

USER 65532:65532
WORKDIR /nonexistent
EXPOSE 8787
ENTRYPOINT ["/usr/local/bin/arqen", "mcp-server"]
