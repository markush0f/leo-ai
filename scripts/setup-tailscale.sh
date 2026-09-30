#!/usr/bin/env bash
# Install Tailscale on this Linux host and privately serve Ira over the tailnet.
set -euo pipefail

if [[ "$(uname -s)" != Linux ]]; then
  printf 'Este instalador requiere Linux. Instala Tailscale desde https://tailscale.com/download\n' >&2
  exit 1
fi

for command in curl; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'falta el comando requerido: %s\n' "$command" >&2
    exit 1
  fi
done

as_root() {
  if [[ "$(id -u)" -eq 0 ]]; then
    "$@"
  else
    command -v sudo >/dev/null 2>&1 || {
      printf 'se requieren permisos root; instala sudo o ejecuta con sudo\n' >&2
      exit 1
    }
    sudo "$@"
  fi
}

if ! command -v tailscale >/dev/null 2>&1; then
  installer="$(mktemp)"
  trap 'rm -f "$installer"' EXIT
  printf 'Descargando instalador oficial de Tailscale...\n'
  curl --proto '=https' --tlsv1.2 -fsSL https://tailscale.com/install.sh -o "$installer"
  as_root sh "$installer"
fi

if command -v systemctl >/dev/null 2>&1; then
  as_root systemctl enable --now tailscaled
fi

printf 'Conectando esta máquina a tu tailnet...\n'
as_root tailscale up

printf 'Publicando Ira solo dentro de tu tailnet...\n'
as_root tailscale serve --bg http://127.0.0.1:8787

printf '\nTailscale listo. Estado y URL HTTPS:\n'
as_root tailscale serve status
