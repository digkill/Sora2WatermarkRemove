-- Dev-only seed data for Lava webhook testing.
-- Run manually: psql "$DATABASE_URL" -f scripts/seed_dev.sql

INSERT INTO users (username, email, password_hash, credits, monthly_quota)
VALUES ('lava_test', 'test@lava.top', '$2b$12$KIXQ1O6pPrdYwKjvJ3uNQe9w4f3bZr3u4hO4bV5bqS6V4FQ8m9cPS', 0, 0)
ON CONFLICT (email) DO NOTHING;

UPDATE products
SET lava_offer_id = 'd31384b8-e412-4be5-a2ec-297ae6666c8f'
WHERE slug = 'pack_5';

UPDATE products
SET lava_offer_id = '11111111-1111-1111-1111-111111111111'
WHERE slug = 'sub_basic';

INSERT INTO transactions (
    user_id,
    product_id,
    provider,
    provider_order_id,
    provider_parent_order_id,
    amount,
    currency,
    status,
    type,
    payload
)
SELECT
    u.id,
    p.id,
    'lava',
    '7ea82675-4ded-4133-95a7-a6efbaf165cc',
    NULL,
    p.price,
    p.currency,
    'pending',
    'payment',
    '{}'::jsonb
FROM users u
JOIN products p ON p.slug = 'pack_5'
WHERE u.email = 'test@lava.top'
ON CONFLICT (provider, provider_order_id) DO NOTHING;

INSERT INTO transactions (
    user_id,
    product_id,
    provider,
    provider_order_id,
    provider_parent_order_id,
    amount,
    currency,
    status,
    type,
    payload
)
SELECT
    u.id,
    p.id,
    'lava',
    'sub-contract-1',
    'sub-contract-1',
    p.price,
    p.currency,
    'pending',
    'payment',
    '{}'::jsonb
FROM users u
JOIN products p ON p.slug = 'sub_basic'
WHERE u.email = 'test@lava.top'
ON CONFLICT (provider, provider_order_id) DO NOTHING;
