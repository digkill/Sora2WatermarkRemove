# Sora Clean Backend

A production-ready Rust + Actix Web backend for removing watermarks from Sora videos. It supports user authentication, credit-based billing, Lava.top payments, KIE processing, S3 storage, webhooks, and a RabbitMQ-based status checker.

This repository also includes a Next.js frontend in `frontend/`.

## Features

- Email + password auth with email verification
- JWT-protected API
- One-time credits, monthly quotas, and free first generation flag
- S3 storage for original and cleaned videos
- KIE.ai integration for watermark removal (webhook + polling fallback)
- Lava.top payments (one-time + subscriptions)
- RabbitMQ queue for status checks
- OpenAPI docs at `/docs`

## Tech Stack

- Rust 1.75+ / Actix Web 4
- PostgreSQL 16
- SQLx migrations
- Amazon S3 (or S3-compatible)
- KIE.ai API
- Lava.top API
- RabbitMQ (optional but recommended)
- Next.js + TypeScript + Tailwind + shadcn (frontend)

## Project Structure

```
.
├── migrations/            # SQLx migrations
├── src/                   # Rust backend
├── tests/                 # Integration/unit tests
├── frontend/              # Next.js frontend
└── Readme.md
```

## Quick Start (Backend)

1) Install Rust and SQLx CLI

```
cargo install sqlx-cli --no-default-features --features rustls,postgres
```

2) Configure environment

Copy `.env.example` to `.env` and fill values.

3) Run migrations

```
sqlx migrate run
```

4) Start server

```
RUST_LOG=info cargo run
```

Server runs on `http://localhost:8065`.

## Quick Start (Frontend)

```
cd frontend
npm install
npm run dev
```

Configure `frontend/.env.local`:

```
NEXT_PUBLIC_API_BASE_URL=http://localhost:8065
NEXT_PUBLIC_SITE_URL=http://localhost:3000
NEXT_PUBLIC_DISABLE_SUBSCRIPTIONS=true
```

## Environment Variables

See `.env.example` for the full list. Key values:

- `DATABASE_URL` / `TEST_DATABASE_URL`
- `JWT_SECRET`
- `KIE_API_KEY` / `KIE_API_BASE_URL`
- `LAVA_API_KEY` / `LAVA_WEBHOOK_KEY`
- `S3_BUCKET` / `S3_ENDPOINT` / `S3_PUBLIC_BASE_URL`
- `CALLBACK_BASE_URL` / `APP_BASE_URL`
- `CORS_ALLOWED_ORIGINS`
- `DISABLE_SUBSCRIPTIONS`
- `RABBITMQ_URL` / `KIE_STATUS_POLL_INTERVAL_SECS`

## API Overview

### Auth
- `POST /auth/register` (email, password, username?)
- `POST /auth/login`
- `GET /auth/verify?token=...`
- `POST /auth/resend-verification`

### Uploads
- `POST /api/upload` (multipart: file or url)
- `GET /api/uploads?limit=100&offset=0`
- `GET /api/credits`

### Products / Payments
- `GET /api/products`
- `POST /api/create-payment`

### Subscriptions
- `GET /api/subscriptions`
- `POST /api/subscriptions/cancel`

### Webhooks
- `POST /api/watermark-callback` (KIE)
- `POST /callback/api/watermark-callback` (KIE alias)
- `POST /webhook/lava` (Lava.top)

## KIE Integration

You can send either a file or a URL. If a URL is provided, the backend forwards it directly to KIE.

KIE callback can include:
- `outputUrl`
- or `resultJson` with `resultUrls`

Both are supported.

## RabbitMQ Status Queue

The worker polls `uploads` with `status='processing'`, sends tasks to `kie.status.check`, and updates status based on KIE `recordInfo`:

- `success` -> `ready` + `cleaned_url`
- `fail` -> `failed`

## Frontend

The frontend includes:

- Login / Register / Verify
- Dashboard with credits, packs, and subscriptions
- Generate page with upload or URL input
- Recent uploads list with download links

## Notes

- `DISABLE_SUBSCRIPTIONS=true` hides subscription products in backend and frontend.
- `MOCK_S3=true` bypasses S3 for testing.
- If KIE callbacks are blocked by auth, ensure webhook routes are outside `/api` scope.

## License

MIT
