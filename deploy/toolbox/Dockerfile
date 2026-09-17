# MCP Toolbox, built from the local checkout at third_party/mcp-toolbox.
# Compose build context is third_party/ (see docker-compose.yml).
FROM golang:1.26-bookworm AS build

WORKDIR /src
COPY mcp-toolbox/ .

ENV CGO_ENABLED=1
ENV GOTOOLCHAIN=auto

RUN apt-get update \
    && apt-get install -y --no-install-recommends gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*

RUN go build -trimpath -ldflags="-s -w" -o /out/toolbox .

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 1000 --create-home toolbox

COPY --from=build --chown=toolbox:toolbox /out/toolbox /toolbox

USER toolbox
WORKDIR /home/toolbox
EXPOSE 5000

HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=12 \
    CMD curl -fsS http://127.0.0.1:5000/healthz

ENTRYPOINT ["/toolbox"]
CMD ["--prebuilt=postgres", "--address=0.0.0.0"]
