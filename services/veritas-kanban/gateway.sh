#!/bin/sh
set -eu
exec supergateway \
  --stdio "node /src/mcp/dist/index.js" \
  --outputTransport streamableHttp \
  --stateful \
  --port "${PORT:-3100}" \
  --streamableHttpPath /mcp \
  --logLevel info \
  < /dev/null
