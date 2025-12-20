// src/api/products.rs

use actix_web::{HttpResponse, Responder, get, web};

use crate::{AppState, db};

#[get("/products")]
pub async fn list_products(state: web::Data<AppState>) -> impl Responder {
    match db::list_active_products(&state.pool).await {
        Ok(mut products) => {
            if std::env::var("DISABLE_SUBSCRIPTIONS").unwrap_or_default() == "true" {
                products.retain(|p| p.product_type != "subscription");
            }
            HttpResponse::Ok().json(products)
        }
        Err(e) => {
            eprintln!("list_products db error: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}
