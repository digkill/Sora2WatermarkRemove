#!/usr/bin/env bash
set -euo pipefail

if [[ "$(id -u)" != "0" ]]; then
  echo "Run as root (sudo)." >&2
  exit 1
fi

APP_DIR="${APP_DIR:-/opt/sora-watermark-remove}"
SERVICE_NAME="${SERVICE_NAME:-sora_watermark_remove}"
NGINX_SITE="${NGINX_SITE:-sora_watermark_remove}"
STATIC_DIR="${STATIC_DIR:-/var/www/sora-watermark-remove}"
PURGE="${PURGE:-0}"

echo "==> Stopping service"
systemctl disable --now "${SERVICE_NAME}" 2>/dev/null || true
rm -f "/etc/systemd/system/${SERVICE_NAME}.service"
systemctl daemon-reload

echo "==> Removing nginx site"
rm -f "/etc/nginx/sites-enabled/${NGINX_SITE}.conf"
rm -f "/etc/nginx/sites-available/${NGINX_SITE}.conf"
nginx -t && systemctl reload nginx || true

if [[ "${PURGE}" == "1" ]]; then
  echo "==> Purging files"
  rm -rf "${APP_DIR}" "${STATIC_DIR}"
fi

echo "Done."
