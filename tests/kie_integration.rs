use actix_web::test::TestRequest;
use actix_web::{App, HttpMessage, test, web};
use actix_web::dev::Service;
use httpmock::Method::POST;
use httpmock::{Mock, MockServer};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use sora_watermark_remov::api::handlers::upload;
use sora_watermark_remov::api::webhooks::watermark_callback;

mod support;

fn set_env(key: &str, value: &str) {
    unsafe {
        std::env::set_var(key, value);
    }
}

fn build_multipart_body(boundary: &str, field_name: &str, filename: &str, content_type: &str, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{field_name}\"; filename=\"{filename}\"\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(data);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

#[actix_web::test]
async fn upload_sends_kie_request_and_returns_task() {
    set_env("MOCK_S3", "true");
    let server = MockServer::start_async().await;
    set_env("KIE_API_BASE_URL", &server.url(""));

    let mock: Mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/jobs/createTask")
            .header("Authorization", "Bearer test-kie");
        then.status(200)
            .json_body(json!({
                "data": { "taskId": "task-123" }
            }));
    });

    let test_db = support::init_test_db().await;
    let pool = &test_db.pool;

    let user_id: i32 = sqlx::query(
        r#"INSERT INTO users (username, email, password_hash, credits, monthly_quota)
           VALUES ($1, $2, $3, 1, 0)
           RETURNING id"#,
    )
    .bind("kie_user")
    .bind(format!("kie_test_{}@example.com", Uuid::new_v4()))
    .bind("hash")
    .fetch_one(pool)
    .await
    .expect("insert user")
    .get("id");

    let state = web::Data::new(support::build_state(test_db.pool.clone(), "test-key").await);
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .wrap_fn(move |req, srv| {
                req.extensions_mut().insert(user_id);
                let fut = srv.call(req);
                async move { fut.await }
            })
            .service(upload),
    )
    .await;

    let boundary = "BOUNDARY";
    let body = build_multipart_body(
        boundary,
        "file",
        "video.mp4",
        "video/mp4",
        b"fake-bytes",
    );

    let req = TestRequest::post()
        .uri("/upload")
        .insert_header(("content-type", format!("multipart/form-data; boundary={boundary}")))
        .set_payload(body)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json body");
    assert_eq!(json["task_id"], "task-123");
    mock.assert();
}

#[actix_web::test]
async fn watermark_callback_marks_upload_ready() {
    set_env("MOCK_S3", "true");
    let server = MockServer::start_async().await;

    let file_mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/file.mp4");
        then.status(200).body("mock");
    });

    let test_db = support::init_test_db().await;
    let pool = &test_db.pool;

    let user_id: i32 = sqlx::query(
        r#"INSERT INTO users (username, email, password_hash, credits, monthly_quota)
           VALUES ($1, $2, $3, 0, 0)
           RETURNING id"#,
    )
    .bind("kie_user")
    .bind(format!("kie_cb_{}@example.com", Uuid::new_v4()))
    .bind("hash")
    .fetch_one(pool)
    .await
    .expect("insert user")
    .get("id");

    let task_id = "task-callback-1";
    let _ = sqlx::query(
        r#"INSERT INTO uploads (user_id, original_filename, original_s3_key, status, task_id)
           VALUES ($1, $2, $3, 'processing', $4)"#,
    )
    .bind(user_id)
    .bind("original.mp4")
    .bind("original/key.mp4")
    .bind(task_id)
    .execute(pool)
    .await
    .expect("insert upload");

    let state = web::Data::new(support::build_state(test_db.pool.clone(), "test-key").await);
    let app = test::init_service(App::new().app_data(state.clone()).service(watermark_callback)).await;

    let payload = json!({
        "code": 200,
        "data": {
            "taskId": task_id,
            "status": "success",
            "outputUrl": format!("{}/file.mp4", server.url(""))
        }
    });

    let req = TestRequest::post()
        .uri("/api/watermark-callback")
        .set_json(payload)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let row = sqlx::query("SELECT status, cleaned_url FROM uploads WHERE task_id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .expect("select upload");
    let status: String = row.get("status");
    let cleaned_url: String = row.get("cleaned_url");
    assert_eq!(status, "ready");
    assert!(cleaned_url.ends_with("/file.mp4"));
}
