#!/bin/sh
set -eu
exec supergateway \
  --stdio "node /app/dist/mcp.js" \
  --outputTransport streamableHttp \
  --stateful \
  --port "${PORT:-3201}" \
  --streamableHttpPath /mcp \
  --logLevel info \
  < /dev/null
