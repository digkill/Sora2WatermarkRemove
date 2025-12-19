// /api/subscriptions
pub async fn list_subscriptions(user: web::ReqData<i32>, state: web::Data<AppState>) -> impl Responder {
    let subs = sqlx::query!("SELECT * FROM subscriptions WHERE user_id = $1", *user)
        .fetch_all(&state.pool)
        .await
        .unwrap();

    HttpResponse::Ok().json(subs)
}

// /api/subscriptions/cancel
pub async fn cancel_subscription(/* subscription_id */, state: web::Data<AppState>) -> impl Responder {
    // Можно просто пометить как canceled
    sqlx::query!("UPDATE subscriptions SET status = 'canceled', canceled_at = NOW() WHERE id = $1 AND user_id = $2", id, user_id)
        .execute(&state.pool)
        .await?;

    HttpResponse::Ok().body("Subscription canceled")
}