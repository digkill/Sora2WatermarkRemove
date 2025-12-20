// src/api/subscriptions.rs

use actix_web::{HttpResponse, Responder, get, post, web};
use serde::Deserialize;

use crate::{AppState, db, models::Subscription};

#[get("/subscriptions")]
pub async fn list_subscriptions(
    state: web::Data<AppState>,
    user_id: web::ReqData<i32>,
) -> impl Responder {
    let user_id = *user_id;

    if std::env::var("DISABLE_SUBSCRIPTIONS").unwrap_or_default() == "true" {
        return HttpResponse::Ok().json(Vec::<Subscription>::new());
    }

    match db::list_user_subscriptions(&state.pool, user_id).await {
        Ok(subs) => HttpResponse::Ok().json(subs),
        Err(e) => {
            eprintln!("list_subscriptions db error: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CancelSubscriptionRequest {
    pub subscription_id: i32,
}

#[post("/subscriptions/cancel")]
pub async fn cancel_subscription(
    state: web::Data<AppState>,
    user_id: web::ReqData<i32>,
    payload: web::Json<CancelSubscriptionRequest>,
) -> impl Responder {
    let user_id = *user_id;

    if std::env::var("DISABLE_SUBSCRIPTIONS").unwrap_or_default() == "true" {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "subscriptions are temporarily disabled"
        }));
    }

    match db::cancel_user_subscription(&state.pool, user_id, payload.subscription_id).await {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"status": "canceled"})),
        Err(e) => {
            eprintln!("cancel_subscription db error: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}
