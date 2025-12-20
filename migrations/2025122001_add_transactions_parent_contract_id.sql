-- Add parent contract id for recurring payments (Lava subscriptions)

ALTER TABLE transactions
    ADD COLUMN provider_parent_order_id VARCHAR(255);

CREATE INDEX IF NOT EXISTS idx_transactions_provider_parent_order
    ON transactions(provider, provider_parent_order_id);
