-- Store buyer email used in Lava for subscription management

ALTER TABLE subscriptions
    ADD COLUMN buyer_email VARCHAR(255);

CREATE INDEX IF NOT EXISTS idx_subscriptions_buyer_email
    ON subscriptions(buyer_email);
