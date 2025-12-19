// src/api/payments.rs
use actix_web::{post, web, HttpResponse};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::api::config::Config;
use serde_json::json;

#[derive(Deserialize)]
pub struct CreatePaymentRequest {
    pub price: f64,
    pub currency: String,
}

#[derive(Serialize)]
struct LavaCreateLink {
    amount: f64,
    currency: String,
}

#[post("/api/create-payment")]
pub async fn create_payment(
    state: web::Data<AppState>,
    user: web::ReqData<i32>,
    body: web::Json<serde_json::Value>,
) -> impl Responder {
    let product_slug: String = body["product_slug"].as_str().unwrap_or("").to_string();
    let is_recurring = body["recurring"].as_bool().unwrap_or(false);

    let product = sqlx::query!(
        "SELECT price, name FROM products WHERE slug = $1 AND is_active = true",
        product_slug
    )
        .fetch_optional(&state.pool)
        .await
        .unwrap();

    let product = match product {
        Some(p) => p,
        None => return HttpResponse::BadRequest().body("Invalid product"),
    };

    let order_id = format!("order-{}-{}", user, Uuid::new_v4());

    let lava_payload = json!({
        "sum": product.price,
        "orderId": order_id,
        "projectId": std::env::var("LAVA_PROJECT_ID").unwrap(),
        "successUrl": format!("{}/payment-success", state.callback_base_url),
        "failUrl": format!("{}/payment-fail", state.callback_base_url),
        "hookUrl": format!("{}/webhook/lava", state.callback_base_url),
        "customFields": json!({
            "user_id": user,
            "product_slug": product_slug,
            "recurring": is_recurring
        }).to_string()
    });

    // Подпись (по документации Lava)
    let signature = /* рассчитывается по secret key */;

    HttpResponse::Ok().json(json!({
        "payment_url": format!("https://lava.top/pay?{}", /* параметры + sign */)
    }))
}
