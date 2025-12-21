#!/usr/bin/env bash
set -euo pipefail

if [[ "$(id -u)" != "0" ]]; then
  echo "Run as root (sudo)." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

APP_DIR="${APP_DIR:-/opt/sora-watermark-remove}"
SERVICE_NAME="${SERVICE_NAME:-sora_watermark_remove}"
RUN_USER="${RUN_USER:-www-data}"
RUN_GROUP="${RUN_GROUP:-www-data}"
STATIC_DIR="${STATIC_DIR:-/var/www/sora-watermark-remove}"
NGINX_SITE="${NGINX_SITE:-sora_watermark_remove}"
BACKEND_PORT="${BACKEND_PORT:-8065}"

APP_DOMAIN="${APP_DOMAIN:-app.example.com}"
API_DOMAIN="${API_DOMAIN:-api.example.com}"
USE_SPLIT="${USE_SPLIT:-1}"
ENABLE_SSL="${ENABLE_SSL:-0}"

SKIP_BUILD="${SKIP_BUILD:-0}"

echo "==> Installing system packages"
apt-get update -y
apt-get install -y nginx certbot

echo "==> Preparing directories"
mkdir -p "${APP_DIR}" "${STATIC_DIR}"

echo "==> Syncing project to ${APP_DIR}"
rsync -a --delete --exclude target --exclude frontend/node_modules "${REPO_DIR}/" "${APP_DIR}/"

if [[ "${SKIP_BUILD}" != "1" ]]; then
  echo "==> Building backend"
  (cd "${APP_DIR}" && cargo build --release)

  echo "==> Building frontend (static export)"
  (cd "${APP_DIR}/frontend" && yarn install && yarn build)

  echo "==> Publishing frontend"
  rm -rf "${STATIC_DIR:?}/"*
  cp -R "${APP_DIR}/frontend/out/"* "${STATIC_DIR}/"
fi

echo "==> Writing systemd unit"
cat > "/etc/systemd/system/${SERVICE_NAME}.service" <<EOF
[Unit]
Description=Sora Watermark Remover API
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${RUN_USER}
Group=${RUN_GROUP}
WorkingDirectory=${APP_DIR}
EnvironmentFile=${APP_DIR}/.env
Environment=RUST_LOG=info
ExecStart=${APP_DIR}/target/release/sora_watermark_remove
Restart=always
RestartSec=3
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

echo "==> Enabling service"
systemctl daemon-reload
systemctl enable --now "${SERVICE_NAME}"

echo "==> Writing nginx config"
if [[ "${ENABLE_SSL}" == "1" ]]; then
  if [[ "${USE_SPLIT}" == "1" ]]; then
    TEMPLATE="${REPO_DIR}/deploy/nginx/sora_watermark_remove_ssl_split.conf"
  else
    TEMPLATE="${REPO_DIR}/deploy/nginx/sora_watermark_remove_ssl.conf"
  fi
else
  TEMPLATE="${REPO_DIR}/deploy/nginx/sora_watermark_remove.conf"
fi

sed \
  -e "s/app.example.com/${APP_DOMAIN}/g" \
  -e "s/api.example.com/${API_DOMAIN}/g" \
  -e "s/example.com/${APP_DOMAIN}/g" \
  -e "s|/var/www/sora-watermark-remove|${STATIC_DIR}|g" \
  -e "s|127.0.0.1:8065|127.0.0.1:${BACKEND_PORT}|g" \
  "${TEMPLATE}" > "/etc/nginx/sites-available/${NGINX_SITE}.conf"

ln -sf "/etc/nginx/sites-available/${NGINX_SITE}.conf" "/etc/nginx/sites-enabled/${NGINX_SITE}.conf"

nginx -t
systemctl reload nginx

cat <<EOF
Done.
- Service: ${SERVICE_NAME}
- Nginx site: ${NGINX_SITE}
- App dir: ${APP_DIR}
- Static dir: ${STATIC_DIR}
EOF
