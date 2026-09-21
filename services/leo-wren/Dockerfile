FROM python:3.13-slim

WORKDIR /app

COPY pyproject.toml README.md ./
COPY src ./src
RUN python -m pip install --no-cache-dir .

COPY . ./
COPY docker-entrypoint.sh /usr/local/bin/leo-wren-entrypoint
RUN chmod +x /usr/local/bin/leo-wren-entrypoint

ENTRYPOINT ["leo-wren-entrypoint"]
CMD ["check"]
