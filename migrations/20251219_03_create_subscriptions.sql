-- Активные подписки пользователей
CREATE TABLE subscriptions (
                               id SERIAL PRIMARY KEY,
                               user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                               product_id INTEGER NOT NULL REFERENCES products(id),

                               provider VARCHAR(50) NOT NULL,                   -- 'lava', 'yookassa' и т.д.
                               provider_subscription_id VARCHAR(255),           -- внешний ID подписки у провайдера (для отмены/продления)

                               status VARCHAR(30) NOT NULL DEFAULT 'active',    -- active, canceled, past_due, expired
                               current_period_start TIMESTAMP WITH TIME ZONE,
                               current_period_end TIMESTAMP WITH TIME ZONE,     -- дата окончания текущего периода

                               created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                               updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                               canceled_at TIMESTAMP WITH TIME ZONE
);

CREATE UNIQUE INDEX idx_subscriptions_provider_id ON subscriptions(provider, provider_subscription_id);
CREATE INDEX idx_subscriptions_user_id ON subscriptions(user_id);
CREATE INDEX idx_subscriptions_status ON subscriptions(status);