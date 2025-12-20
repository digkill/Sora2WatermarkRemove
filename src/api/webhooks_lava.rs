// src/api/webhooks_lava.rs

use actix_web::{HttpRequest, HttpResponse, post, web};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::Value;
use sqlx::Row;
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::{AppState, billing, db};

/// Важно: точный payload Lava может отличаться.
/// Мы поддерживаем минимум:
/// - orderId / order_id / contractId
/// - status (succeeded/failed) или paid=true
/// - parentContractId для регулярных платежей
/// - customFields с user_id/product_slug если Lava их возвращает
#[derive(Debug, Deserialize, ToSchema)]
pub struct LavaWebhook {
    /// Тип события от Lava: например, "payment_result" или "recurring_payment"
    #[serde(default, rename = "type", alias = "eventType", alias = "event_type")]
    pub event_type: Option<String>,

    #[serde(alias = "orderId", alias = "order_id", alias = "contractId", alias = "contract_id")]
    pub order_id: Option<String>,

    #[serde(alias = "parentContractId", alias = "parent_contract_id")]
    pub parent_order_id: Option<String>,

    pub status: Option<String>,

    pub paid: Option<bool>,

    #[serde(default)]
    pub amount: Option<String>,

    #[serde(default)]
    pub currency: Option<String>,

    #[serde(default, rename = "customFields", alias = "custom_fields")]
    pub custom_fields: Option<String>,

    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Debug)]
pub struct NormalizedLavaWebhook {
    pub event_type: Option<String>,
    pub order_id: Option<String>,
    pub parent_order_id: Option<String>,
    pub status: Option<String>,
    pub paid: Option<bool>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub buyer_email: Option<String>,
    pub product_offer_id: Option<String>,
    pub custom_fields: Option<Value>,
    pub raw: Value,
}

fn is_succeeded(payload: &NormalizedLavaWebhook) -> bool {
    if payload.paid.unwrap_or(false) {
        return true;
    }
    matches!(
        payload.status.as_deref(),
        Some("succeeded")
            | Some("success")
            | Some("paid")
            | Some("completed")
            | Some("done")
    )
}

fn is_failed(payload: &NormalizedLavaWebhook) -> bool {
    matches!(
        payload.status.as_deref(),
        Some("failed")
            | Some("fail")
            | Some("canceled")
            | Some("cancelled")
            | Some("error")
            | Some("expired")
            | Some("declined")
    )
}

fn is_event_success(event_type: Option<&str>) -> bool {
    matches!(
        event_type,
        Some("payment.success") | Some("subscription.recurring.payment.success")
    )
}

fn is_event_failed(event_type: Option<&str>) -> bool {
    matches!(
        event_type,
        Some("payment.failed") | Some("subscription.recurring.payment.failed")
    )
}

fn is_event_cancelled(event_type: Option<&str>) -> bool {
    matches!(event_type, Some("subscription.cancelled"))
}

fn is_event_payment(event_type: Option<&str>) -> bool {
    matches!(event_type, Some("payment.success") | Some("payment.failed"))
}

fn is_event_recurring(event_type: Option<&str>) -> bool {
    matches!(
        event_type,
        Some("subscription.recurring.payment.success")
            | Some("subscription.recurring.payment.failed")
    )
}

pub fn parse_webhook_body(body: &[u8]) -> Result<Value, String> {
    if body.is_empty() {
        return Err("empty body".to_string());
    }

    if let Ok(json) = serde_json::from_slice::<Value>(body) {
        return Ok(json);
    }

    let form: HashMap<String, String> =
        serde_urlencoded::from_bytes(body).map_err(|e| format!("invalid body: {e}"))?;
    let mut map = serde_json::Map::new();
    for (k, v) in form {
        map.insert(k, Value::String(v));
    }
    Ok(Value::Object(map))
}

fn get_nested<'a>(raw: &'a Value, key: &str) -> Option<&'a Value> {
    if let Some(v) = raw.get(key) {
        return Some(v);
    }
    for container in ["data", "payload"] {
        if let Some(v) = raw.get(container).and_then(|v| v.get(key)) {
            return Some(v);
        }
    }
    None
}

fn get_nested_path<'a>(raw: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = raw;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn extract_string(raw: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = get_nested(raw, key) {
            if let Some(s) = value_to_string(v) {
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
    }
    None
}

fn extract_bool(raw: &Value, keys: &[&str]) -> Option<bool> {
    for key in keys {
        if let Some(v) = get_nested(raw, key) {
            if let Some(b) = v.as_bool() {
                return Some(b);
            }
            if let Some(s) = v.as_str() {
                let s = s.to_lowercase();
                if s == "true" || s == "1" || s == "yes" {
                    return Some(true);
                }
                if s == "false" || s == "0" || s == "no" {
                    return Some(false);
                }
            }
        }
    }
    None
}

fn extract_custom_fields(raw: &Value) -> Option<Value> {
    let value = get_nested(raw, "customFields").or_else(|| get_nested(raw, "custom_fields"))?;
    match value {
        Value::String(s) => {
            if s.trim_start().starts_with('{') || s.trim_start().starts_with('[') {
                serde_json::from_str::<Value>(s).ok().or_else(|| Some(Value::String(s.clone())))
            } else {
                Some(Value::String(s.clone()))
            }
        }
        _ => Some(value.clone()),
    }
}

pub fn normalize_payload(raw: Value) -> NormalizedLavaWebhook {
    let event_type = extract_string(&raw, &["type", "eventType", "event_type", "event"])
        .map(|value| value.to_lowercase());
    let order_id = extract_string(
        &raw,
        &[
            "orderId",
            "order_id",
            "contractId",
            "contract_id",
            "invoiceId",
            "invoice_id",
            "paymentId",
            "payment_id",
            "id",
        ],
    );
    let parent_order_id = extract_string(
        &raw,
        &[
            "parentContractId",
            "parent_contract_id",
            "parentOrderId",
            "parent_order_id",
        ],
    );
    let buyer_email = get_nested_path(&raw, &["buyer", "email"])
        .and_then(value_to_string)
        .or_else(|| extract_string(&raw, &["buyerEmail", "buyer_email", "email"]));
    let product_offer_id = get_nested_path(&raw, &["product", "id"])
        .and_then(value_to_string)
        .or_else(|| extract_string(&raw, &["productId", "product_id", "offerId", "offer_id"]));
    let status = extract_string(&raw, &["status", "paymentStatus", "payment_status", "result"]);
    let paid = extract_bool(&raw, &["paid", "isPaid", "success"]);
    let amount = extract_string(&raw, &["amount", "sum", "price"]);
    let currency = extract_string(&raw, &["currency"]);
    let custom_fields = extract_custom_fields(&raw);

    NormalizedLavaWebhook {
        event_type,
        order_id,
        parent_order_id,
        status: status.map(|s| s.to_lowercase()),
        paid,
        amount,
        currency: currency.map(|c| c.to_uppercase()),
        buyer_email,
        product_offer_id,
        custom_fields,
        raw,
    }
}

pub fn extract_api_key(req: &HttpRequest, payload: &Value) -> Option<String> {
    if let Some(header) = req.headers().get("X-Api-Key").and_then(|v| v.to_str().ok()) {
        return Some(header.to_string());
    }

    if let Ok(query) = serde_urlencoded::from_str::<HashMap<String, String>>(req.query_string()) {
        if let Some(key) = query.get("api_key").or_else(|| query.get("apiKey")) {
            return Some(key.clone());
        }
    }

    extract_string(payload, &["apiKey", "api_key", "key"])
}

#[post("/webhook/lava")]
#[utoipa::path(
    post,
    path = "/webhook/lava",
    tag = "webhooks",
    request_body = LavaWebhook,
    params(
        ("X-Api-Key" = String, Header, description = "Shared secret from Lava settings")
    ),
    responses(
        (status = 200, description = "Webhook processed"),
        (status = 401, description = "Invalid API key")
    )
)]
pub async fn lava_webhook(
    body: web::Bytes,
    req: HttpRequest,
    state: web::Data<AppState>,
) -> HttpResponse {
    let raw_payload = match parse_webhook_body(&body) {
        Ok(payload) => payload,
        Err(e) => {
            eprintln!("lava_webhook invalid payload: {e}");
            return HttpResponse::BadRequest().finish();
        }
    };

    let provided_key = extract_api_key(&req, &raw_payload);
    if provided_key.as_deref() != Some(state.lava_webhook_key.as_str()) {
        return HttpResponse::Unauthorized().finish();
    }

    let payload = normalize_payload(raw_payload);

    let provider = "lava";
    let provider_order_id = payload.order_id.clone();
    let provider_parent_order_id = payload.parent_order_id.clone();
    let event_type = payload.event_type.as_deref();

    eprintln!(
        "lava_webhook payload: buyer_email={:?} product_id={:?} contract_id={:?}",
        payload.buyer_email, payload.product_offer_id, provider_order_id
    );

    if is_event_cancelled(event_type) {
        let contract_id = provider_parent_order_id
            .as_deref()
            .or_else(|| provider_order_id.as_deref());

        if let Some(contract_id) = contract_id {
            let _ = sqlx::query(
                r#"UPDATE subscriptions
                   SET status = 'canceled', canceled_at = NOW()
                   WHERE provider = $1 AND provider_subscription_id = $2"#,
            )
            .bind(provider)
            .bind(contract_id)
            .execute(&state.pool)
            .await;
        }

        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "canceled": true}));
    }

    let tx_row = if let Some(order_id) = provider_order_id.as_deref() {
        match sqlx::query(
            r#"SELECT id, user_id, product_id, status, amount::text as amount, currency
               FROM transactions
               WHERE provider = $1 AND provider_order_id = $2"#,
        )
        .bind(provider)
        .bind(order_id)
        .fetch_optional(&state.pool)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("lava_webhook select tx error: {e}");
                return HttpResponse::InternalServerError().finish();
            }
        }
    } else {
        None
    };

    let parent_row = if tx_row.is_none() {
        if let Some(parent_id) = provider_parent_order_id.as_deref() {
            match sqlx::query(
                r#"SELECT id, user_id, product_id, status, amount::text as amount, currency, provider_order_id
                   FROM transactions
                   WHERE provider = $1 AND (provider_order_id = $2 OR provider_parent_order_id = $2)"#,
            )
            .bind(provider)
            .bind(parent_id)
            .fetch_optional(&state.pool)
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("lava_webhook select parent tx error: {e}");
                    return HttpResponse::InternalServerError().finish();
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let tx_row = tx_row.or(parent_row);

    let is_success = is_event_success(event_type) || is_succeeded(&payload);
    let is_failed = is_event_failed(event_type) || is_failed(&payload);

    let tx_row = match tx_row {
        Some(row) => row,
        None => {
            if !is_success {
                return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
            }

            let Some(contract_id) = provider_order_id.as_deref() else {
                return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
            };

            let user_id = if let Some(custom_fields) = payload.custom_fields.as_ref() {
                custom_fields
                    .get("user_id")
                    .and_then(value_to_string)
                    .and_then(|v| v.parse::<i32>().ok())
            } else {
                None
            };

            let user_id = if let Some(user_id) = user_id {
                Some(user_id)
            } else if let Some(email) = payload.buyer_email.as_deref() {
                match sqlx::query("SELECT id FROM users WHERE email = $1")
                    .bind(email)
                    .fetch_optional(&state.pool)
                    .await
                {
                    Ok(Some(r)) => Some(r.get::<i32, _>("id")),
                    Ok(None) => None,
                    Err(e) => {
                        eprintln!("lava_webhook select user by email error: {e}");
                        return HttpResponse::InternalServerError().finish();
                    }
                }
            } else {
                None
            };

            let Some(user_id) = user_id else {
                return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
            };

            let product_slug = payload
                .custom_fields
                .as_ref()
                .and_then(|fields| fields.get("product_slug"))
                .and_then(value_to_string);

            let product_row = if let Some(product_offer_id) = payload.product_offer_id.as_deref() {
                match sqlx::query(
                    r#"SELECT id, price::text as price, currency, product_type
                       FROM products
                       WHERE lava_offer_id::text = $1"#,
                )
                .bind(product_offer_id)
                .fetch_optional(&state.pool)
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("lava_webhook select product by offer id error: {e}");
                        return HttpResponse::InternalServerError().finish();
                    }
                }
            } else if let Some(product_slug) = product_slug.as_deref() {
                match sqlx::query(
                    r#"SELECT id, price::text as price, currency, product_type
                       FROM products
                       WHERE slug = $1"#,
                )
                .bind(product_slug)
                .fetch_optional(&state.pool)
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("lava_webhook select product by slug error: {e}");
                        return HttpResponse::InternalServerError().finish();
                    }
                }
            } else {
                None
            };

            let Some(product_row) = product_row else {
                return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
            };

            let product_id: i32 = product_row.get("id");
            let product_type: String = product_row.get("product_type");

            if is_event_recurring(event_type) && product_type != "subscription" {
                eprintln!(
                    "lava_webhook mismatch: recurring event for non-subscription product_id={}",
                    product_id
                );
                return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
            }

            if is_event_payment(event_type) && product_type == "subscription" {
                // payment.* может быть первым платежом подписки — разрешаем
            } else if is_event_payment(event_type) && product_type != "one_time" {
                eprintln!(
                    "lava_webhook mismatch: payment event for unexpected product_id={}",
                    product_id
                );
                return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
            }

            let amount = payload
                .amount
                .clone()
                .unwrap_or_else(|| product_row.get::<String, _>("price"));
            let currency = payload
                .currency
                .clone()
                .unwrap_or_else(|| product_row.get::<String, _>("currency"));

            let insert_result = match sqlx::query(
                r#"INSERT INTO transactions
                   (user_id, product_id, provider, provider_order_id, provider_parent_order_id, amount, currency, status, type, payload)
                   VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, 'pending', 'payment', $8)
                   ON CONFLICT (provider, provider_order_id) DO NOTHING
                   RETURNING id"#,
            )
            .bind(user_id)
            .bind(product_id)
            .bind(provider)
            .bind(contract_id)
            .bind(provider_parent_order_id.as_deref())
            .bind(amount)
            .bind(currency)
            .bind(payload.raw.clone())
            .fetch_optional(&state.pool)
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("lava_webhook insert tx from webhook error: {e}");
                    return HttpResponse::InternalServerError().finish();
                }
            };

            let _ = insert_result;
            let tx_row = match sqlx::query(
                r#"SELECT id, user_id, product_id, status, amount::text as amount, currency
                   FROM transactions
                   WHERE provider = $1 AND provider_order_id = $2"#,
            )
            .bind(provider)
            .bind(contract_id)
            .fetch_optional(&state.pool)
            .await
            {
                Ok(Some(row)) => row,
                Ok(None) => {
                    return HttpResponse::Ok()
                        .json(serde_json::json!({"ok": true, "ignored": true}));
                }
                Err(e) => {
                    eprintln!("lava_webhook select tx after insert error: {e}");
                    return HttpResponse::InternalServerError().finish();
                }
            };

            tx_row
        }
    };

    let mut tx_id: i32 = tx_row.get("id");
    let user_id: i32 = tx_row.get("user_id");
    let product_id: Option<i32> = tx_row.get("product_id");
    let mut current_status: String = tx_row.get("status");
    let mut created_tx = false;

    if current_status == "succeeded"
        && provider_parent_order_id.is_some()
        && provider_order_id
            .as_deref()
            .is_some_and(|order_id| {
                tx_row
                    .try_get::<String, _>("provider_order_id")
                    .map(|existing| existing != order_id)
                    .unwrap_or(false)
            })
    {
        if let Some(order_id) = provider_order_id.as_deref() {
            let amount = payload
                .amount
                .clone()
                .unwrap_or_else(|| tx_row.get::<String, _>("amount"));
            let currency = payload
                .currency
                .clone()
                .unwrap_or_else(|| tx_row.get::<String, _>("currency"));

            let insert_row = match sqlx::query(
                r#"INSERT INTO transactions
                   (user_id, product_id, provider, provider_order_id, provider_parent_order_id, amount, currency, status, type, payload)
                   VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, 'pending', 'payment', $8)
                   RETURNING id"#,
            )
            .bind(user_id)
            .bind(product_id)
            .bind(provider)
            .bind(order_id)
            .bind(provider_parent_order_id.as_deref())
            .bind(amount)
            .bind(currency)
            .bind(payload.raw.clone())
            .fetch_one(&state.pool)
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("lava_webhook insert recurring tx error: {e}");
                    return HttpResponse::InternalServerError().finish();
                }
            };

            tx_id = insert_row.get("id");
            current_status = "pending".to_string();
            created_tx = true;
        }
    }

    if !created_tx && (current_status == "succeeded" || current_status == "failed") {
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "idempotent": true}));
    }

    if is_failed {
        let _ = sqlx::query(
            r#"UPDATE transactions
               SET status = 'failed',
                   provider_parent_order_id = COALESCE(provider_parent_order_id, $1),
                   payload = COALESCE(payload, '{}'::jsonb) || $2::jsonb
               WHERE id = $3"#,
        )
        .bind(provider_parent_order_id.as_deref())
        .bind(payload.raw.clone())
        .bind(tx_id)
        .execute(&state.pool)
        .await;

        return HttpResponse::Ok().json(serde_json::json!({"ok": true}));
    }

    if !is_success {
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
    }

    let paid_at = Utc::now();
    let _ = sqlx::query(
        r#"UPDATE transactions
           SET status = 'succeeded', paid_at = $1,
               provider_parent_order_id = COALESCE(provider_parent_order_id, $2),
               payload = COALESCE(payload, '{}'::jsonb) || $3::jsonb
           WHERE id = $4"#,
    )
    .bind(paid_at)
    .bind(provider_parent_order_id.as_deref())
    .bind(payload.raw.clone())
    .bind(tx_id)
    .execute(&state.pool)
    .await;

    let Some(product_id) = product_id else {
        return HttpResponse::Ok().json(serde_json::json!({"ok": true}));
    };

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

    if is_event_recurring(event_type) && product_type != "subscription" {
        eprintln!(
            "lava_webhook mismatch: recurring event for non-subscription product_id={}",
            product_id
        );
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
    }

    if is_event_payment(event_type) && product_type == "subscription" {
        // payment.* может быть первым платежом подписки — разрешаем
    } else if is_event_payment(event_type) && product_type != "one_time" {
        eprintln!(
            "lava_webhook mismatch: payment event for unexpected product_id={}",
            product_id
        );
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "ignored": true}));
    }

    if product_type == "one_time" {
        if let Some(c) = credits_granted {
            if let Err(e) = billing::grant_one_time_credits(&state.pool, user_id, c).await {
                eprintln!("grant_one_time_credits error: {e}");
                return HttpResponse::InternalServerError().finish();
            }
        }
        return HttpResponse::Ok().json(serde_json::json!({"ok": true}));
    }

    let period_start = paid_at;
    let period_end = paid_at + Duration::days(30);
    let provider_subscription_id = provider_parent_order_id
        .as_deref()
        .or_else(|| provider_order_id.as_deref());

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

    let _ = sqlx::query("UPDATE transactions SET subscription_id = $1 WHERE id = $2")
        .bind(sub_id)
        .bind(tx_id)
        .execute(&state.pool)
        .await;

    if let Some(mc) = monthly_credits {
        if let Err(e) = billing::set_subscription_monthly_quota(&state.pool, user_id, mc).await {
            eprintln!("set_subscription_monthly_quota error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    }

    HttpResponse::Ok().json(serde_json::json!({"ok": true}))
}
