FROM node:22-bookworm-slim AS frontend
WORKDIR /app/desktop
COPY desktop/package*.json ./
RUN npm ci
COPY desktop/ ./
RUN npm run build

FROM node:22-bookworm-slim AS docs
WORKDIR /app/docs-app
COPY docs-app/package*.json ./
RUN npm ci
COPY docs-app/ ./
RUN npm run build

FROM rust:1-bookworm AS rust-build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY deploy ./deploy
COPY desktop/src-tauri ./desktop/src-tauri
COPY --from=frontend /app/desktop/dist ./desktop/dist
COPY --from=docs /app/desktop/public/docs ./desktop/public/docs
RUN cargo build --release -p ira-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl libgomp1 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 ira
WORKDIR /app
COPY --from=rust-build /app/target/release/ira-server /usr/local/bin/ira-server
COPY --from=rust-build /app/desktop/dist ./desktop/dist
COPY --from=rust-build /app/desktop/public/docs ./desktop/dist/docs
ENV IRA_HTTP_BIND=0.0.0.0:8787 \
    IRA_WEB_ROOT=/app/desktop/dist \
    HOME=/home/ira
USER ira
EXPOSE 8787
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=5 \
  CMD ["curl", "-fsS", "http://127.0.0.1:8787/healthz"]
CMD ["ira-server"]
