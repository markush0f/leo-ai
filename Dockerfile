FROM node:22-bookworm-slim AS frontend
WORKDIR /app/desktop
COPY desktop/package*.json ./
RUN npm ci
COPY desktop/ ./
ARG VITE_IRA_REALTIME=http://127.0.0.1:8790/realtime
ENV VITE_IRA_REALTIME=$VITE_IRA_REALTIME
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
COPY deploy/ira-entrypoint.sh /usr/local/bin/ira-entrypoint
RUN chmod 755 /usr/local/bin/ira-entrypoint
ENV IRA_HTTP_BIND=0.0.0.0:8787 \
    IRA_WEB_ROOT=/app/desktop/dist \
    HOME=/home/ira
USER ira
ENTRYPOINT ["/usr/local/bin/ira-entrypoint"]
EXPOSE 8787
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=5 \
  CMD ["curl", "-fsS", "http://127.0.0.1:8787/api/health"]
CMD ["ira-server"]
