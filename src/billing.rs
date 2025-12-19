// src/billing.rs

use chrono::{Utc, Duration};
use sqlx::{PgPool, FromRow};
use crate::db::User;

/// Обновляет месячную квоту пользователя на основе активной подписки
pub async fn refresh_monthly_quota(pool: &PgPool, user_id: i32) -> Result<(), sqlx::Error> {
    let now = Utc::now();

    // Получаем пользователя и его активную подписку
    let record = sqlx::query!(
        r#"
        SELECT
            u.credits, u.monthly_quota, u.quota_reset_at,
            p.monthly_credits
        FROM users u
        LEFT JOIN subscriptions s ON s.user_id = u.id AND s.status = 'active'
        LEFT JOIN products p ON p.id = s.product_id AND p.product_type = 'subscription'
        WHERE u.id = $1
        "#,
        user_id
    )
        .fetch_optional(pool)
        .await?;

    let (mut current_quota, reset_at, monthly_credits) = match record {
        Some(r) => (r.monthly_quota.unwrap_or(0), r.quota_reset_at, r.monthly_credits),
        None => return Ok(()),
    };

    let should_reset = match reset_at {
        Some(reset) => now >= reset,
        None => true, // если никогда не сбрасывали — сбросим сейчас
    };

    if should_reset {
        current_quota = monthly_credits.unwrap_or(0);
        let next_reset = (now + Duration::days(30)).date_naive().and_hms_opt(0, 0, 0); // приблизительно месяц

        sqlx::query!(
            "UPDATE users SET monthly_quota = $1, quota_reset_at = $2 WHERE id = $3",
            current_quota,
            next_reset.map(|d| d.and_utc()),
            user_id
        )
            .execute(pool)
            .await?;
    }

    Ok(())
}

/// Проверяет, может ли пользователь выполнить удаление водяного знака
/// Возвращает тип использованного кредита: 'one_time' или 'subscription'
pub async fn can_remove_watermark(pool: &PgPool, user_id: i32) -> Result<Option<String>, sqlx::Error> {
    refresh_monthly_quota(pool, user_id).await?;

    let user = sqlx::query_as::<_, User>("SELECT credits, monthly_quota FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;

    if user.monthly_quota > 0 {
        return Ok(Some("subscription".to_string()));
    }
    if user.credits > 0 {
        return Ok(Some("one_time".to_string()));
    }

    Ok(None)
}

/// Списывает один кредит/квоту после успешной загрузки
pub async fn consume_credit(pool: &PgPool, user_id: i32, credit_type: &str) -> Result<(), sqlx::Error> {
    match credit_type {
        "subscription" => {
            sqlx::query!("UPDATE users SET monthly_quota = monthly_quota - 1 WHERE id = $1", user_id)
                .execute(pool)
                .await?;
        }
        "one_time" => {
            sqlx::query!("UPDATE users SET credits = credits - 1 WHERE id = $1", user_id)
                .execute(pool)
                .await?;
        }
        _ => {}
    }
    Ok(())
}