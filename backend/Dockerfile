# Builder: full toolchain + sqlite headers (sqlx links system sqlite).
FROM rust:1-bookworm AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libsqlite3-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
# Compile dependencies once so code changes rebuild only the app.
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && : > src/lib.rs \
    && cargo build --release \
    && rm -rf src
COPY src ./src
COPY prompts ./prompts
# COPY preserves the files' original mtimes (older than the stub build),
# which would make cargo skip the rebuild: touch everything first. This
# also keeps prompt edits (compiled in via include_str!) effective.
RUN find src prompts -exec touch -c {} + && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libsqlite3-0 \
    && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /data
WORKDIR /app
COPY --from=builder /build/target/release/kidlearn-backend /app/kidlearn-backend
ENV PORT=3000 \
    DATABASE_URL=sqlite:///data/kidlearn.db?mode=rwc \
    RUST_LOG=info
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD curl -fsS http://127.0.0.1:${PORT:-3000}/health || exit 1
CMD ["/app/kidlearn-backend"]
