// src/api/webhooks.rs

use crate::{AppState, s3_utils::build_public_url};
use actix_web::{HttpResponse, post, web};
use aws_sdk_s3::primitives::ByteStream;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, ToSchema)] // Добавили Debug
pub struct CallbackPayload {
    code: i32,
    #[allow(dead_code)]
    msg: Option<String>,
    data: CallbackData,
}

#[derive(Deserialize, Debug, ToSchema)]
pub struct CallbackData {
    #[serde(rename = "taskId")]
    task_id: String,
    status: String,
    #[serde(rename = "outputUrl")]
    output_url: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/watermark-callback",
    tag = "webhooks",
    request_body = CallbackPayload,
    responses(
        (status = 200, description = "Callback processed"),
        (status = 400, description = "Task failed or invalid payload"),
        (status = 500, description = "Server error")
    )
)]
#[post("/api/watermark-callback")]
pub async fn watermark_callback(
    payload: web::Json<CallbackPayload>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let payload = payload.into_inner();

    if payload.code != 200 || payload.data.status != "success" {
        eprintln!("Kie.ai error: {:?}", payload);
        return HttpResponse::BadRequest().body("Task failed");
    }

    let cleaned_url = match payload.data.output_url {
        Some(url) => url,
        None => return HttpResponse::BadRequest().body("No output URL"),
    };

    let task_id = payload.data.task_id.clone();
    let s3_key = format!("cleaned/{}.mp4", task_id);

    let resp = match HttpClient::new().get(&cleaned_url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            eprintln!("Download failed: {}", r.status());
            return HttpResponse::InternalServerError().finish();
        }
        Err(e) => {
            eprintln!("Request error: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Bytes error: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let stream = ByteStream::from(bytes);

    if let Err(e) = state
        .s3_client
        .put_object()
        .bucket(&state.s3_bucket)
        .key(&s3_key)
        .content_type("video/mp4")
        .body(stream)
        .send()
        .await
    {
        eprintln!("S3 upload failed: {}", e);
        return HttpResponse::InternalServerError().finish();
    }

    // Обновляем БД (runtime query, чтобы сборка не зависела от наличия таблиц в DEV БД)
    let _ = sqlx::query(
        "UPDATE uploads SET cleaned_s3_key = $1, cleaned_url = $2, status = 'ready' WHERE task_id = $3",
    )
    .bind(&s3_key)
    .bind(build_public_url(&state.s3_public_base_url, &state.s3_bucket, &s3_key))
    .bind(&task_id)
    .execute(&state.pool)
    .await;

    HttpResponse::Ok().body("OK")
}
