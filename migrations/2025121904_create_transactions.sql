-- Payment transactions
CREATE TABLE transactions (
                              id SERIAL PRIMARY KEY,
                              user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                              product_id INTEGER REFERENCES products(id),
                              subscription_id INTEGER REFERENCES subscriptions(id),

                              provider VARCHAR(50) NOT NULL,
                              provider_order_id VARCHAR(255) NOT NULL,

                              amount DECIMAL(10,2) NOT NULL,
                              currency VARCHAR(3) NOT NULL DEFAULT 'USD',

                              status VARCHAR(30) NOT NULL CHECK (status IN ('pending', 'succeeded', 'failed', 'refunded')),
                              type VARCHAR(20) NOT NULL CHECK (type IN ('payment', 'refund')),

                              payload JSONB,
                              paid_at TIMESTAMP WITH TIME ZONE,

                              created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_transactions_provider_order
    ON transactions(provider, provider_order_id);

CREATE INDEX idx_transactions_user_id ON transactions(user_id);
CREATE INDEX idx_transactions_status ON transactions(status);