// src/api/webhooks_lava.rs

use actix_web::{post, web, HttpResponse};
use chrono::{Duration, Utc};
use serde::Deserialize;
use sqlx::Row;

use crate::{billing, db, AppState};

/// Важно: точный payload Lava может отличаться.
/// Мы поддерживаем минимум:
/// - orderId / order_id
/// - status (succeeded/failed) или paid=true
/// - customFields с user_id/product_slug если Lava их возвращает
#[derive(Debug, Deserialize)]
pub struct LavaWebhook {
    #[serde(alias = "orderId", alias = "order_id")]
    pub order_id: String,

    pub status: Option<String>,

    pub paid: Option<bool>,

    #[serde(default)]
    pub amount: Option<String>,

    #[serde(default)]
    pub currency: Option<String>,

    #[serde(default)]
    pub customFields: Option<String>,

    #[serde(flatten)]
    pub extra: serde_json::Value,
}

fn is_succeeded(payload: &LavaWebhook) -> bool {
    if payload.paid.unwrap_or(false) {
        return true;
    }
    matches!(payload.status.as_deref(), Some("succeeded") | Some("success") | Some("paid"))
}

fn is_failed(payload: &LavaWebhook) -> bool {
    matches!(payload.status.as_deref(), Some("failed") | Some("fail") | Some("canceled"))
}

#[post("/webhook/lava")]
pub async fn lava_webhook(payload: web::Json<LavaWebhook>, state: web::Data<AppState>) -> HttpResponse {
    let payload = payload.into_inner();

    let provider = "lava";
    let provider_order_id = payload.order_id.clone();

    // Найти transaction
    let tx_row = match sqlx::query(
        r#"SELECT id, user_id, product_id, status
           FROM transactions
           WHERE provider = $1 AND provider_order_id = $2"#,
    )
    .bind(provider)
    .bind(&provider_order_id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("lava_webhook select tx error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let Some(tx_row) = tx_row else {
        // неизвестный orderId — считаем OK, чтобы Lava не ретраила бесконечно
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
    };

    let tx_id: i32 = tx_row.get("id");
    let user_id: i32 = tx_row.get("user_id");
    let product_id: Option<i32> = tx_row.get("product_id");
    let current_status: String = tx_row.get("status");

    // Идемпотентность: если уже succeeded/failed — ничего не делаем
    if current_status == "succeeded" || current_status == "failed" {
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "idempotent": true}));
    }

    if is_failed(&payload) {
        let _ = sqlx::query(
            r#"UPDATE transactions
               SET status = 'failed', payload = COALESCE(payload, '{}'::jsonb) || $1::jsonb
               WHERE id = $2"#,
        )
        .bind(payload.extra)
        .bind(tx_id)
        .execute(&state.pool)
        .await;

        return HttpResponse::Ok().json(serde_json::json!({"ok": true}));
    }

    if !is_succeeded(&payload) {
        // неизвестный статус — всё равно 200
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
    }

    // succeeded
    let paid_at = Utc::now();
    let _ = sqlx::query(
        r#"UPDATE transactions
           SET status = 'succeeded', paid_at = $1,
               payload = COALESCE(payload, '{}'::jsonb) || $2::jsonb
           WHERE id = $3"#,
    )
    .bind(paid_at)
    .bind(payload.extra.clone())
    .bind(tx_id)
    .execute(&state.pool)
    .await;

    // Если нет product_id — всё равно ок
    let Some(product_id) = product_id else {
        return HttpResponse::Ok().json(serde_json::json!({"ok": true}));
    };

    // Загружаем продукт и применяем бизнес-логику
    let product_row = match sqlx::query(
        r#"SELECT product_type, credits_granted, monthly_credits
           FROM products
           WHERE id = $1"#,
    )
    .bind(product_id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("lava_webhook select product error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let Some(product_row) = product_row else {
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "missing_product": true}));
    };

    let product_type: String = product_row.get("product_type");
    let credits_granted: Option<i32> = product_row.get("credits_granted");
    let monthly_credits: Option<i32> = product_row.get("monthly_credits");

    if product_type == "one_time" {
        if let Some(c) = credits_granted {
            if let Err(e) = billing::grant_one_time_credits(&state.pool, user_id, c).await {
                eprintln!("grant_one_time_credits error: {e}");
                return HttpResponse::InternalServerError().finish();
            }
        }
        return HttpResponse::Ok().json(serde_json::json!({"ok": true}));
    }

    // subscription
    let period_start = paid_at;
    let period_end = paid_at + Duration::days(30);
    let provider_subscription_id: Option<&str> = None; // можно заполнить, если Lava отдаёт

    let sub_id = match db::upsert_subscription_active(
        &state.pool,
        user_id,
        product_id,
        provider,
        provider_subscription_id,
        period_start,
        period_end,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("upsert_subscription_active error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    // привяжем transaction к subscription
    let _ = sqlx::query("UPDATE transactions SET subscription_id = $1 WHERE id = $2")
        .bind(sub_id)
        .bind(tx_id)
        .execute(&state.pool)
        .await;

    // выставим месячную квоту
    if let Some(mc) = monthly_credits {
        if let Err(e) = billing::set_subscription_monthly_quota(&state.pool, user_id, mc).await {
            eprintln!("set_subscription_monthly_quota error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    }

    HttpResponse::Ok().json(serde_json::json!({"ok": true}))
}
