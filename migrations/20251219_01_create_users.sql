-- Users table
CREATE TABLE users (
                       id SERIAL PRIMARY KEY,
                       username VARCHAR(255) UNIQUE NOT NULL,
                       email VARCHAR(255) UNIQUE NOT NULL,
                       password_hash VARCHAR(255) NOT NULL,
                       credits INTEGER NOT NULL DEFAULT 1,                    -- 1 free removal on signup
                       monthly_quota INTEGER NOT NULL DEFAULT 0,             -- monthly allowance from subscription
                       quota_reset_at TIMESTAMP WITH TIME ZONE,               -- when monthly quota resets
                       is_active BOOLEAN NOT NULL DEFAULT true,
                       created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                       updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_email ON users(email);