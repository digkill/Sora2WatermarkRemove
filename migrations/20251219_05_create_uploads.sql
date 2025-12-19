-- Загрузки и обработки видео
CREATE TABLE uploads (
                         id SERIAL PRIMARY KEY,
                         user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,

                         original_filename VARCHAR(255) NOT NULL,
                         original_s3_key VARCHAR(512) NOT NULL,

                         task_id VARCHAR(255) UNIQUE,
                         cleaned_s3_key VARCHAR(512),
                         cleaned_url TEXT,

                         status VARCHAR(50) NOT NULL DEFAULT 'uploaded',   -- uploaded, processing, ready, failed, paid_pending
                         used_credit_type VARCHAR(20) DEFAULT 'one_time',  -- one_time или subscription

                         lava_order_id VARCHAR(255),                      -- если была разовая оплата через Lava

                         created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                         updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_uploads_user_id ON uploads(user_id);
CREATE INDEX idx_uploads_task_id ON uploads(task_id);
CREATE INDEX idx_uploads_status ON uploads(status);