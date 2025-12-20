use actix_web::test::TestRequest;
use actix_web::{App, test, web};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use sora_watermark_remov::api::webhooks_lava::lava_webhook;

mod support;

#[actix_web::test]
async fn webhook_one_time_payment_success_creates_tx_and_grants_credits() {
    let test_db = support::init_test_db().await;
    let pool = &test_db.pool;
    let suffix = Uuid::new_v4().to_string();
    let email = format!("webhook_test_{suffix}@lava.top");
    let offer_uuid = Uuid::new_v4();
    let contract_id = Uuid::new_v4().to_string();
    let slug = format!("test_pack_{suffix}");

    let user_id: i32 = sqlx::query(
        r#"INSERT INTO users (username, email, password_hash, credits, monthly_quota)
           VALUES ($1, $2, $3, 0, 0)
           RETURNING id"#,
    )
    .bind(format!("user_{suffix}"))
    .bind(&email)
    .bind("test-hash")
    .fetch_one(pool)
    .await
    .expect("insert user")
    .get("id");

    let product_id: i32 = sqlx::query(
        r#"INSERT INTO products
           (slug, name, description, price, currency, product_type, credits_granted, monthly_credits, is_active, lava_offer_id)
           VALUES ($1, $2, $3, 9.99, 'RUB', 'one_time', 3, NULL, true, $4)
           RETURNING id"#,
    )
    .bind(&slug)
    .bind("Test Pack")
    .bind("Test one_time pack")
    .bind(offer_uuid)
    .fetch_one(pool)
    .await
    .expect("insert product")
    .get("id");

    let state = web::Data::new(support::build_state(test_db.pool.clone(), "test-key").await);
    let app = test::init_service(App::new().app_data(state.clone()).service(lava_webhook)).await;

    let payload = json!({
        "eventType": "payment.success",
        "product": {
            "id": offer_uuid.to_string(),
            "title": "Test Pack"
        },
        "buyer": {
            "email": email
        },
        "contractId": contract_id,
        "amount": 9.99,
        "currency": "RUB",
        "timestamp": "2024-02-05T09:38:27.33277Z",
        "status": "completed",
        "errorMessage": ""
    });

    let req = TestRequest::post()
        .uri("/webhook/lava")
        .insert_header(("X-Api-Key", "test-key"))
        .set_json(payload)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let credits: i32 = sqlx::query("SELECT credits FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("select credits")
        .get("credits");
    assert_eq!(credits, 3);

    let status: String = sqlx::query(
        r#"SELECT status FROM transactions
           WHERE provider = 'lava' AND provider_order_id = $1"#,
    )
    .bind(&contract_id)
    .fetch_one(pool)
    .await
    .expect("select tx")
    .get("status");
    assert_eq!(status, "succeeded");

    let _ = sqlx::query("DELETE FROM transactions WHERE provider = 'lava' AND provider_order_id = $1")
        .bind(&contract_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM products WHERE id = $1")
        .bind(product_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}

#[actix_web::test]
async fn webhook_recurring_payment_success_creates_tx_and_updates_subscription() {
    let test_db = support::init_test_db().await;
    let pool = &test_db.pool;
    let suffix = Uuid::new_v4().to_string();
    let email = format!("webhook_sub_{suffix}@lava.top");
    let offer_uuid = Uuid::new_v4();
    let parent_contract_id = format!("parent-{suffix}");
    let child_contract_id = format!("child-{suffix}");
    let slug = format!("test_sub_{suffix}");

    let user_id: i32 = sqlx::query(
        r#"INSERT INTO users (username, email, password_hash, credits, monthly_quota)
           VALUES ($1, $2, $3, 0, 0)
           RETURNING id"#,
    )
    .bind(format!("user_{suffix}"))
    .bind(&email)
    .bind("test-hash")
    .fetch_one(pool)
    .await
    .expect("insert user")
    .get("id");

    let product_id: i32 = sqlx::query(
        r#"INSERT INTO products
           (slug, name, description, price, currency, product_type, credits_granted, monthly_credits, is_active, lava_offer_id)
           VALUES ($1, $2, $3, 19.99, 'RUB', 'subscription', NULL, 12, true, $4)
           RETURNING id"#,
    )
    .bind(&slug)
    .bind("Test Sub")
    .bind("Test subscription")
    .bind(offer_uuid)
    .fetch_one(pool)
    .await
    .expect("insert product")
    .get("id");

    let _ = sqlx::query(
        r#"INSERT INTO transactions
           (user_id, product_id, provider, provider_order_id, provider_parent_order_id, amount, currency, status, type, payload)
           VALUES ($1, $2, 'lava', $3, $3, 19.99, 'RUB', 'succeeded', 'payment', '{}'::jsonb)"#,
    )
    .bind(user_id)
    .bind(product_id)
    .bind(&parent_contract_id)
    .execute(pool)
    .await
    .expect("insert parent tx");

    let state = web::Data::new(support::build_state(test_db.pool.clone(), "test-key").await);
    let app = test::init_service(App::new().app_data(state.clone()).service(lava_webhook)).await;

    let payload = json!({
        "eventType": "subscription.recurring.payment.success",
        "product": {
            "id": offer_uuid.to_string(),
            "title": "Test Sub"
        },
        "buyer": {
            "email": email
        },
        "contractId": child_contract_id,
        "parentContractId": parent_contract_id,
        "amount": 19.99,
        "currency": "RUB",
        "timestamp": "2024-02-05T09:38:27.33277Z",
        "status": "completed",
        "errorMessage": ""
    });

    let req = TestRequest::post()
        .uri("/webhook/lava")
        .insert_header(("X-Api-Key", "test-key"))
        .set_json(payload)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let status: String = sqlx::query(
        r#"SELECT status FROM transactions
           WHERE provider = 'lava' AND provider_order_id = $1"#,
    )
    .bind(&child_contract_id)
    .fetch_one(pool)
    .await
    .expect("select child tx")
    .get("status");
    assert_eq!(status, "succeeded");

    let sub_row = sqlx::query(
        r#"SELECT status FROM subscriptions
           WHERE provider = 'lava' AND provider_subscription_id = $1"#,
    )
    .bind(&parent_contract_id)
    .fetch_one(pool)
    .await
    .expect("select subscription");
    let sub_status: String = sub_row.get("status");
    assert_eq!(sub_status, "active");

    let monthly_quota: i32 = sqlx::query("SELECT monthly_quota FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("select quota")
        .get("monthly_quota");
    assert_eq!(monthly_quota, 12);

    let _ = sqlx::query(
        "DELETE FROM transactions WHERE provider = 'lava' AND provider_order_id IN ($1, $2)",
    )
    .bind(&parent_contract_id)
    .bind(&child_contract_id)
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "DELETE FROM subscriptions WHERE provider = 'lava' AND provider_subscription_id = $1",
    )
    .bind(&parent_contract_id)
    .execute(pool)
    .await;
    let _ = sqlx::query("DELETE FROM products WHERE id = $1")
        .bind(product_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await;
}
