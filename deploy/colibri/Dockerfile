FROM debian:bookworm-slim AS download

ARG COLIBRI_VERSION=1.11.0
ARG COLIBRI_SHA256=01bca809309f0647d32998c7d6f0bf9fa2f65fe0c395b2c72f4701dbd5b375ad

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*
RUN curl -fsSL \
    "https://github.com/JustVugg/colibri/releases/download/v${COLIBRI_VERSION}/colibri-v${COLIBRI_VERSION}-linux-x86_64.tar.gz" \
    -o /tmp/colibri.tar.gz && \
    echo "${COLIBRI_SHA256}  /tmp/colibri.tar.gz" | sha256sum -c - && \
    mkdir /opt/colibri && \
    tar -xzf /tmp/colibri.tar.gz -C /opt/colibri

FROM debian:bookworm-slim AS runtime

RUN apt-get update && \
    apt-get install -y --no-install-recommends libgomp1 python3 && \
    rm -rf /var/lib/apt/lists/* && \
    useradd --system --uid 1000 --create-home colibri
COPY --from=download --chown=colibri:colibri /opt/colibri /app

WORKDIR /app
USER colibri
EXPOSE 5000
ENV COLI_MODEL=/model \
    PYTHONUNBUFFERED=1

ENTRYPOINT ["python3", "/app/coli"]
CMD ["info"]
