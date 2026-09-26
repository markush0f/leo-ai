# Colibrì 1.11.0 (JustVugg/colibri), CPU engines only.
# The converted model is mounted at /model and is not part of this image.
# Tag matches docker-compose image ira-ai-colibri:1.11.0.

FROM debian:stable-slim AS build
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential ca-certificates git make && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /src
RUN git clone --depth 1 --branch v1.11.0 https://github.com/JustVugg/colibri.git .
# One binary per family. coli resolves the artifact next to itself.
# ARCH stays at the Makefile default (native on this build machine).
RUN make -C c -j"$(nproc)" \
        colibri olmoe glm53 inkling kimi_k3 qwen36 qwen38 deepseek_v41 deepseek-v4

FROM debian:stable-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends python3 libgomp1 && \
    rm -rf /var/lib/apt/lists/* && \
    useradd -m -u 1000 colibri
WORKDIR /app
COPY --from=build \
    /src/c/coli \
    /src/c/version.py \
    /src/c/openai_server.py \
    /src/c/v4_dsml.py \
    /src/c/v41_dsml.py \
    /src/c/family_registry.py \
    /src/c/resource_plan.py \
    /src/c/doctor.py \
    /src/c/autotune.py \
    /app/
COPY --from=build \
    /src/c/colibri \
    /src/c/olmoe \
    /src/c/glm53 \
    /src/c/inkling \
    /src/c/kimi_k3 \
    /src/c/qwen36 \
    /src/c/qwen38 \
    /src/c/deepseek_v4 \
    /src/c/deepseek_v41 \
    /app/
COPY --from=build /src/c/tools /app/tools
ENV COLI_MODEL=/model \
    PYTHONUNBUFFERED=1
VOLUME ["/model"]
EXPOSE 5000
USER colibri
ENTRYPOINT ["python3", "/app/coli"]
CMD ["info"]
