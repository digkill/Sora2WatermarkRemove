-- Таблица загруженных видео и их обработки
CREATE TABLE uploads (
                         id SERIAL PRIMARY KEY,
                         user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,

                         original_filename VARCHAR(255) NOT NULL,
                         original_s3_key VARCHAR(512) NOT NULL,       -- например: uploads/original/12345.mp4

                         task_id VARCHAR(255) UNIQUE,                  -- taskId от Kie.ai (может быть NULL пока не запущено)
                         cleaned_s3_key VARCHAR(512),                  -- cleaned/abc123.mp4
                         cleaned_url TEXT,                            -- публичная ссылка на готовое видео

                         status VARCHAR(50) NOT NULL DEFAULT 'uploaded',  -- uploaded, processing, ready, failed, paid_pending
                         lava_order_id VARCHAR(255),                  -- ID заказа в Lava.top (если был платный)

                         created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
                         updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Индексы для быстрого поиска
CREATE INDEX idx_uploads_user_id ON uploads(user_id);
CREATE INDEX idx_uploads_task_id ON uploads(task_id);
CREATE INDEX idx_uploads_lava_order_id ON uploads(lava_order_id);
CREATE INDEX idx_uploads_status ON uploads(status);