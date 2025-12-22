// src/api/payments.rs

use actix_web::{HttpResponse, Responder, post, web};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use crate::{AppState, api::lava_client, db};

#[derive(Debug, Deserialize)]
pub struct CreatePaymentRequest {
    /// Наш внутренний продукт/тариф
    pub product_slug: String,

    /// Email покупателя в Lava (обязательное поле Lava API)
    /// Источник: users.email (из БД).
    pub buyer_email: Option<String>,

    /// Можно явно задать периодичность. Для подписок по умолчанию MONTHLY.
    /// Для one_time по умолчанию ONE_TIME.
    pub periodicity: Option<String>,

    /// Опционально: провайдер оплаты (UNLIMINT/PAYPAL/STRIPE/...) согласно Lava API.
    pub payment_provider: Option<String>,

    /// Опционально: метод оплаты (CARD/SBP/PIX/...) согласно Lava API.
    pub payment_method: Option<String>,
}

/// Создаёт контракт (invoice) в Lava через Public API (POST /api/v3/invoice)
/// и возвращает ссылку на оплату `paymentUrl`.
///
/// Мы сохраняем contractId (uuid) в `transactions.provider_order_id`.
#[post("/create-payment")]
pub async fn create_payment(
    state: web::Data<AppState>,
    user_id: web::ReqData<i32>,
    payload: web::Json<CreatePaymentRequest>,
) -> impl Responder {
    let user_id = *user_id;

    // 1) загрузим продукт из нашей БД
    let product = match db::get_product_by_slug(&state.pool, &payload.product_slug).await {
        Ok(Some(p)) => p,
        Ok(None) => return HttpResponse::BadRequest().json(json!({"error": "invalid product"})),
        Err(e) => {
            eprintln!("get_product_by_slug error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    if product.product_type == "subscription"
        && std::env::var("DISABLE_SUBSCRIPTIONS").unwrap_or_default() == "true"
    {
        return HttpResponse::BadRequest().json(json!({
            "error": "subscriptions are temporarily disabled"
        }));
    }

    // 2) buyer_email: всегда берём из users.email
    let buyer_email = match sqlx::query("SELECT email FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(r)) => r.get::<String, _>("email"),
        Ok(None) => return HttpResponse::BadRequest().json(json!({"error": "user not found"})),
        Err(e) => {
            eprintln!("select user email error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let buyer_email = buyer_email.trim().to_string();
    if !buyer_email.contains('@') || !buyer_email.contains('.') {
        log::error!("invalid buyer email for lava user_id={} email={}", user_id, buyer_email);
        return HttpResponse::BadRequest().json(json!({"error": "invalid buyer email"}));
    }

    // 3) map internal product_slug -> lava offerId (from DB)
    let offer_id = match product.lava_offer_id.as_deref() {
        Some(id) => id,
        None => {
            return HttpResponse::BadRequest().json(json!({
                "error": "product is not mapped to lava offerId"
            }));
        }
    };

    // 4) periodicity defaulting
    let default_periodicity = if product.product_type == "subscription" {
        "MONTHLY"
    } else {
        "ONE_TIME"
    };

    let periodicity = payload
        .periodicity
        .clone()
        .unwrap_or_else(|| default_periodicity.to_string());

    // 5) create invoice in Lava
    log::info!(
        "lava create invoice user_id={} email={} offer_id={} periodicity={}",
        user_id,
        buyer_email,
        offer_id,
        periodicity
    );
    let invoice = match lava_client::create_invoice_v3(
        &state.lava_api_key,
        lava_client::CreateInvoiceV3Request {
            email: buyer_email.clone(),
            buyer_email: Some(buyer_email.clone()),
            offer_id: offer_id.to_string(),
            currency: product.currency.clone(),
            payment_provider: payload.payment_provider.clone(),
            payment_method: payload.payment_method.clone(),
            periodicity: Some(periodicity),
        },
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            log::error!(
                "lava create_invoice_v3 error: {e} user_id={} email={}",
                user_id,
                buyer_email
            );
            return HttpResponse::BadRequest().json(json!({
                "error": "lava invoice create failed",
                "details": e.to_string()
            }));
        }
    };

    let provider = "lava";
    let provider_order_id = invoice.id.clone();
    let provider_parent_order_id = if product.product_type == "subscription" {
        Some(provider_order_id.clone())
    } else {
        None
    };

    // 6) сохраняем pending transaction (amount/currency берём из нашей product таблицы)
    let tx_payload = json!({
        "user_id": user_id,
        "buyer_email": buyer_email,
        "product_slug": product.slug,
        "product_type": product.product_type,
        "lava_offer_id": offer_id,
        "lava_contract_id": provider_order_id,
        "lava_status": invoice.status,
    });

    let tx_id_row = match sqlx::query(
        r#"INSERT INTO transactions
           (user_id, product_id, provider, provider_order_id, provider_parent_order_id, amount, currency, status, type, payload)
           VALUES ($1, $2, $3, $4, $5, $6::numeric, $7, 'pending', 'payment', $8)
           RETURNING id"#,
    )
    .bind(user_id)
    .bind(product.id)
    .bind(provider)
    .bind(&provider_order_id)
    .bind(provider_parent_order_id)
    .bind(&product.price)
    .bind(&product.currency)
    .bind(tx_payload)
    .fetch_one(&state.pool)
    .await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("create_payment insert tx error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let tx_id: i32 = tx_id_row.get("id");

    HttpResponse::Ok().json(json!({
        "transaction_id": tx_id,
        "provider": provider,
        "provider_order_id": provider_order_id,
        "payment_url": invoice.payment_url
    }))
}
