FROM rust:1.90-bookworm AS builder

WORKDIR /app
COPY . .
RUN cargo build --locked --release -p leo-engine --bin colibri

FROM debian:bookworm-slim AS runtime

RUN useradd --system --uid 10001 --create-home colibri
COPY --from=builder /app/target/release/colibri /usr/local/bin/colibri

USER colibri
EXPOSE 8080
ENV COLIBRI_BIND=0.0.0.0:8080

HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
    CMD ["/usr/local/bin/colibri", "healthcheck"]

ENTRYPOINT ["/usr/local/bin/colibri"]
