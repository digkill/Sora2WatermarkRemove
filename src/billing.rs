// src/billing.rs

use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row};

use crate::db;

/// Обновляет месячную квоту пользователю при наличии активной подписки.
/// Логика:
/// - если есть эффективная подписка (active/canceled, но период не закончился)
/// - и quota_reset_at NULL или <= now
/// тогда ставим monthly_quota = monthly_credits и quota_reset_at = now + 30 дней.
///
/// Примечание: мы используем фиксированные 30 дней (без привязки к календарным месяцам),
/// т.к. в таблице subscriptions хранится период, а провайдер может присылать точные даты.
pub async fn refresh_monthly_quota(pool: &PgPool, user_id: i32) -> Result<(), sqlx::Error> {
    let (sub_opt, monthly_credits) = match db::get_effective_subscription(pool, user_id).await? {
        Some((sub, credits)) => (Some(sub), credits),
        None => (None, 0),
    };

    if sub_opt.is_none() {
        return Ok(());
    }

    let row = sqlx::query("SELECT quota_reset_at FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;

    let quota_reset_at: Option<DateTime<Utc>> = row.get("quota_reset_at");

    let now = Utc::now();
    let need_reset = quota_reset_at.map(|t| t <= now).unwrap_or(true);

    if need_reset {
        let next = now + Duration::days(30);
        sqlx::query("UPDATE users SET monthly_quota = $1, quota_reset_at = $2 WHERE id = $3")
            .bind(monthly_credits)
            .bind(next)
            .bind(user_id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

/// Возвращает тип кредита, который можно списать сейчас:
/// - "monthly" если есть месячная квота (>0)
/// - иначе "one_time" если есть разовые кредиты (>0)
/// - иначе None
pub async fn can_remove_watermark(
    pool: &PgPool,
    user_id: i32,
) -> Result<Option<String>, sqlx::Error> {
    // На всякий случай обновим квоту перед проверкой
    let _ = refresh_monthly_quota(pool, user_id).await;

    let row = sqlx::query("SELECT credits, monthly_quota, free_generation_used FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;

    let credits: i32 = row.get("credits");
    let monthly_quota: i32 = row.get("monthly_quota");
    let free_generation_used: bool = row.get("free_generation_used");

    if monthly_quota > 0 {
        Ok(Some("monthly".to_string()))
    } else if credits > 0 {
        Ok(Some("one_time".to_string()))
    } else if !free_generation_used {
        Ok(Some("free".to_string()))
    } else {
        Ok(None)
    }
}

pub async fn consume_credit(
    pool: &PgPool,
    user_id: i32,
    credit_type: &str,
) -> Result<(), sqlx::Error> {
    match credit_type {
        "monthly" => {
            sqlx::query(
                "UPDATE users SET monthly_quota = GREATEST(monthly_quota - 1, 0) WHERE id = $1",
            )
            .bind(user_id)
            .execute(pool)
            .await?;
        }
        "free" => {
            sqlx::query("UPDATE users SET free_generation_used = true WHERE id = $1")
                .bind(user_id)
                .execute(pool)
                .await?;
        }
        _ => {
            sqlx::query("UPDATE users SET credits = GREATEST(credits - 1, 0) WHERE id = $1")
                .bind(user_id)
                .execute(pool)
                .await?;
        }
    }

    Ok(())
}

pub async fn grant_one_time_credits(
    pool: &PgPool,
    user_id: i32,
    credits: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET credits = credits + $1 WHERE id = $2")
        .bind(credits)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn set_subscription_monthly_quota(
    pool: &PgPool,
    user_id: i32,
    monthly_credits: i32,
) -> Result<(), sqlx::Error> {
    let next = Utc::now() + Duration::days(30);
    sqlx::query("UPDATE users SET monthly_quota = $1, quota_reset_at = $2 WHERE id = $3")
        .bind(monthly_credits)
        .bind(next)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}
