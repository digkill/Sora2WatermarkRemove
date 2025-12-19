// src/api/payments.rs

use actix_web::{post, web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::{db, AppState};

// NOTE: url/signature формат может отличаться от доки Lava — адаптируйте при интеграции.

#[derive(Debug, Deserialize)]
pub struct CreatePaymentRequest {
    pub product_slug: String,
    /// Для подписок: если true, считаем что это recurring
    pub recurring: Option<bool>,
}

/// Упрощённая реализация: мы формируем `provider_order_id` (наш orderId),
/// создаём запись transactions со статусом `pending` и возвращаем URL оплаты.
///
/// Важно: точный формат подписи/URL у Lava может отличаться.
/// Я оставил реализацию через HMAC-SHA256 по канонической строке,
/// чтобы было куда подставить правильный алгоритм из документации.
#[post("/create-payment")]
pub async fn create_payment(
    state: web::Data<AppState>,
    user_id: web::ReqData<i32>,
    payload: web::Json<CreatePaymentRequest>,
) -> impl Responder {
    let user_id = *user_id;

    let product = match db::get_product_by_slug(&state.pool, &payload.product_slug).await {
        Ok(Some(p)) => p,
        Ok(None) => return HttpResponse::BadRequest().json(json!({"error": "invalid product"})),
        Err(e) => {
            eprintln!("get_product_by_slug error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let provider = "lava";
    let provider_order_id = format!("order-{}-{}", user_id, Uuid::new_v4());

    let recurring = payload.recurring.unwrap_or(false);

    // создаём pending transaction
    let amount = product.price.clone();
    let currency = product.currency.clone();

    let tx_payload = json!({
        "user_id": user_id,
        "product_slug": product.slug,
        "product_type": product.product_type,
        "recurring": recurring,
    });

    let res = sqlx::query(
        r#"INSERT INTO transactions (user_id, product_id, provider, provider_order_id, amount, currency, status, type, payload)
           VALUES ($1, $2, $3, $4, $5::numeric, $6, 'pending', 'payment', $7)
           RETURNING id"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(provider)
    .bind(&provider_order_id)
    .bind(&amount)
    .bind(&currency)
    .bind(tx_payload)
    .fetch_one(&state.pool)
    .await;

    let tx_id: i32 = match res {
        Ok(r) => r.get("id"),
        Err(e) => {
            eprintln!("create_payment insert tx error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    // Формируем URL оплаты Lava
    // NOTE: возможно Lava требует другие имена полей.
    let success_url = format!("{}/payment-success", state.callback_base_url);
    let fail_url = format!("{}/payment-fail", state.callback_base_url);
    let hook_url = format!("{}/webhook/lava", state.callback_base_url);

    let project_id = state.lava_project_id.clone();

    // Каноническая строка для подписи (placeholder)
    let sign_base = format!(
        "projectId={}&orderId={}&sum={}&currency={}",
        project_id, provider_order_id, amount, currency
    );
    let signature = crate::api::lava::sign_hmac_sha256_hex(&state.lava_secret_key, &sign_base);

    // Вариант URL (placeholder) — подстроим под фактическую доку Lava
    let payment_url = format!(
        "https://lava.top/pay?projectId={}&orderId={}&sum={}&currency={}&successUrl={}&failUrl={}&hookUrl={}&sign={}",
        urlencoding::encode(&project_id),
        urlencoding::encode(&provider_order_id),
        urlencoding::encode(&amount),
        urlencoding::encode(&currency),
        urlencoding::encode(&success_url),
        urlencoding::encode(&fail_url),
        urlencoding::encode(&hook_url),
        urlencoding::encode(&signature)
    );

    HttpResponse::Ok().json(json!({
        "transaction_id": tx_id,
        "provider": provider,
        "provider_order_id": provider_order_id,
        "payment_url": payment_url
    }))
}
