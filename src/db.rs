// src/db.rs

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

use crate::models::{Product, Subscription};

pub async fn list_active_products(pool: &PgPool) -> Result<Vec<Product>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT id, slug, name, description, price::text as price, currency, product_type,
                  credits_granted, monthly_credits, is_active, created_at
           FROM products
           WHERE is_active = true
           ORDER BY price ASC"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Product {
            id: r.get("id"),
            slug: r.get("slug"),
            name: r.get("name"),
            description: r.get("description"),
            price: r.get("price"),
            currency: r.get("currency"),
            product_type: r.get("product_type"),
            credits_granted: r.get("credits_granted"),
            monthly_credits: r.get("monthly_credits"),
            is_active: r.get("is_active"),
            created_at: r.get("created_at"),
        })
        .collect())
}

pub async fn get_product_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Product>, sqlx::Error> {
    let row = sqlx::query(
        r#"SELECT id, slug, name, description, price::text as price, currency, product_type,
                  credits_granted, monthly_credits, is_active, created_at
           FROM products
           WHERE slug = $1 AND is_active = true"#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| Product {
        id: r.get("id"),
        slug: r.get("slug"),
        name: r.get("name"),
        description: r.get("description"),
        price: r.get("price"),
        currency: r.get("currency"),
        product_type: r.get("product_type"),
        credits_granted: r.get("credits_granted"),
        monthly_credits: r.get("monthly_credits"),
        is_active: r.get("is_active"),
        created_at: r.get("created_at"),
    }))
}

/// Возвращает подписку, которая даёт доступ к квоте прямо сейчас.
/// Важно: `status = 'canceled'` всё ещё считается активной до конца оплаченного периода.
pub async fn get_effective_subscription(
    pool: &PgPool,
    user_id: i32,
) -> Result<Option<(Subscription, i32 /*monthly_credits*/ )>, sqlx::Error> {
    let row = sqlx::query(
        r#"SELECT s.id, s.user_id, s.product_id, s.provider, s.provider_subscription_id,
                  s.status, s.current_period_start, s.current_period_end,
                  s.created_at, s.updated_at, s.canceled_at,
                  p.monthly_credits
           FROM subscriptions s
           JOIN products p ON p.id = s.product_id
           WHERE s.user_id = $1
             AND s.status IN ('active', 'canceled')
             AND (s.current_period_end IS NULL OR s.current_period_end > NOW())
           ORDER BY s.created_at DESC
           LIMIT 1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        let monthly_credits: Option<i32> = r.get("monthly_credits");
        (
            Subscription {
                id: r.get("id"),
                user_id: r.get("user_id"),
                product_id: r.get("product_id"),
                provider: r.get("provider"),
                provider_subscription_id: r.get("provider_subscription_id"),
                status: r.get("status"),
                current_period_start: r.get("current_period_start"),
                current_period_end: r.get("current_period_end"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                canceled_at: r.get("canceled_at"),
            },
            monthly_credits.unwrap_or(0),
        )
    }))
}

pub async fn list_user_subscriptions(pool: &PgPool, user_id: i32) -> Result<Vec<Subscription>, sqlx::Error> {
    let rows = sqlx::query(
        r#"SELECT id, user_id, product_id, provider, provider_subscription_id,
                  status, current_period_start, current_period_end,
                  created_at, updated_at, canceled_at
           FROM subscriptions
           WHERE user_id = $1
           ORDER BY created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Subscription {
            id: r.get("id"),
            user_id: r.get("user_id"),
            product_id: r.get("product_id"),
            provider: r.get("provider"),
            provider_subscription_id: r.get("provider_subscription_id"),
            status: r.get("status"),
            current_period_start: r.get("current_period_start"),
            current_period_end: r.get("current_period_end"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            canceled_at: r.get("canceled_at"),
        })
        .collect())
}

pub async fn cancel_user_subscription(pool: &PgPool, user_id: i32, subscription_id: i32) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE subscriptions
           SET status = 'canceled', canceled_at = NOW()
           WHERE id = $1 AND user_id = $2"#,
    )
    .bind(subscription_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn upsert_subscription_active(
    pool: &PgPool,
    user_id: i32,
    product_id: i32,
    provider: &str,
    provider_subscription_id: Option<&str>,
    current_period_start: DateTime<Utc>,
    current_period_end: DateTime<Utc>,
) -> Result<i32, sqlx::Error> {
    let row = sqlx::query(
        r#"INSERT INTO subscriptions
                (user_id, product_id, provider, provider_subscription_id, status, current_period_start, current_period_end)
           VALUES ($1, $2, $3, $4, 'active', $5, $6)
           ON CONFLICT (provider, provider_subscription_id)
           DO UPDATE SET
               product_id = EXCLUDED.product_id,
               status = 'active',
               current_period_start = EXCLUDED.current_period_start,
               current_period_end = EXCLUDED.current_period_end,
               canceled_at = NULL
           RETURNING id"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(provider)
    .bind(provider_subscription_id)
    .bind(current_period_start)
    .bind(current_period_end)
    .fetch_one(pool)
    .await?;

    Ok(row.get("id"))
}
