CREATE OR REPLACE VIEW user_credits_status AS
SELECT
    id AS user_id,
    credits,
    monthly_quota,
    free_generation_used
FROM users;
