-- Pricing plans / products
CREATE TABLE products (
                          id SERIAL PRIMARY KEY,
                          slug VARCHAR(50) UNIQUE NOT NULL,
                          name VARCHAR(255) NOT NULL,
                          description TEXT,
                          price DECIMAL(10,2) NOT NULL,
                          currency VARCHAR(3) NOT NULL DEFAULT 'USD',
                          product_type VARCHAR(20) NOT NULL CHECK (product_type IN ('one_time', 'subscription')),
                          credits_granted INTEGER,                         -- for one_time packs
                          monthly_credits INTEGER,                         -- for subscriptions
                          is_active BOOLEAN NOT NULL DEFAULT true,
                          created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Seed initial products
INSERT INTO products (slug, name, description, price, currency, product_type, credits_granted, monthly_credits)
VALUES
    ('free', 'Free Starter Pack', '1 free watermark removal on signup', 0.00, 'USD', 'one_time', 1, NULL),
    ('pack_5', '5 Removals Pack', 'Remove watermark from 5 videos', 4.99, 'USD', 'one_time', 5, NULL),
    ('pack_20', '20 Removals Pack', 'Best value for multiple videos', 14.99, 'USD', 'one_time', 20, NULL),
    ('pack_50', '50 Removals Pack', 'For heavy users', 29.99, 'USD', 'one_time', 50, NULL),
    ('sub_basic', 'Basic Monthly', '30 removals per month', 9.99, 'USD', 'subscription', NULL, 30),
    ('sub_pro', 'Pro Monthly', '100 removals per month', 24.99, 'USD', 'subscription', NULL, 100),
    ('sub_unlimited', 'Unlimited Monthly', 'Unlimited removals', 49.99, 'USD', 'subscription', NULL, 9999)
    ON CONFLICT (slug) DO NOTHING;