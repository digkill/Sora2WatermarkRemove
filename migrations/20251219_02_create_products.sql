-- Products / Pricing plans
CREATE TABLE products (
                          id SERIAL PRIMARY KEY,
                          slug VARCHAR(50) UNIQUE NOT NULL,        -- e.g. "pack_5", "sub_monthly"
                          name VARCHAR(255) NOT NULL,
                          description TEXT,
                          price DECIMAL(10,2) NOT NULL,
                          currency VARCHAR(3) DEFAULT 'USD',

    -- Product type
                          product_type VARCHAR(20) NOT NULL CHECK (product_type IN ('one_time', 'subscription')),

    -- For one-time packs
                          credits_granted INTEGER,                 -- how many credits (NULL for subscription)

    -- For subscriptions
                          monthly_credits INTEGER,                 -- how many removals per month (NULL for one-time)

                          is_active BOOLEAN DEFAULT true,
                          created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Seed data: products in English, prices in USD
INSERT INTO products (slug, name, description, price, currency, product_type, credits_granted, monthly_credits) VALUES
                                                                                                                    ('free', 'Free Starter Pack', '1 free watermark removal on signup', 0.00, 'USD', 'one_time', 1, NULL),
                                                                                                                    ('pack_5', '5 Removals Pack', 'Remove watermark from 5 videos', 4.99, 'USD', 'one_time', 5, NULL),
                                                                                                                    ('pack_20', '20 Removals Pack', 'Best value for multiple videos', 14.99, 'USD', 'one_time', 20, NULL),
                                                                                                                    ('pack_50', '50 Removals Pack', 'For heavy users', 29.99, 'USD', 'one_time', 50, NULL),
                                                                                                                    ('sub_basic', 'Basic Monthly Subscription', '30 removals per month', 9.99, 'USD', 'subscription', NULL, 30),
                                                                                                                    ('sub_pro', 'Pro Monthly Subscription', '100 removals per month', 24.99, 'USD', 'subscription', NULL, 100),
                                                                                                                    ('sub_unlimited', 'Unlimited Monthly', 'Unlimited removals every month', 49.99, 'USD', 'subscription', NULL, 9999);  -- practically unlimited