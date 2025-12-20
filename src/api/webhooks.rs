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
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(rename = "outputUrl")]
    output_url: Option<String>,
    #[serde(default)]
    resultJson: Option<String>,
}

async fn handle_watermark_callback(
    payload: web::Json<CallbackPayload>,
    state: web::Data<AppState>,
) -> HttpResponse {
    let payload = payload.into_inner();

    if payload.code != 200 {
        log::warn!("kie callback error payload={:?}", payload);
        return HttpResponse::BadRequest().body("Task failed");
    }

    if let Some(status) = payload.data.status.as_deref() {
        if status != "success" {
            log::warn!("kie callback status not success: {}", status);
            return HttpResponse::BadRequest().body("Task failed");
        }
    }

    if let Some(state) = payload.data.state.as_deref() {
        if state != "success" {
            log::warn!("kie callback state not success: {}", state);
            return HttpResponse::BadRequest().body("Task failed");
        }
    }

    let output_url = if let Some(url) = payload.data.output_url.clone() {
        Some(url)
    } else if let Some(result_json) = payload.data.resultJson.as_deref() {
        match serde_json::from_str::<serde_json::Value>(result_json) {
            Ok(value) => value
                .get("resultUrls")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            Err(e) => {
                log::warn!("kie callback resultJson parse error: {}", e);
                None
            }
        }
    } else {
        None
    };

    if output_url.is_none() {
        log::warn!("kie callback error payload={:?}", payload);
        return HttpResponse::BadRequest().body("Task failed");
    }

    let cleaned_url = output_url.unwrap();

    let task_id = payload.data.task_id.clone();
    let s3_key = format!("cleaned/{}.mp4", task_id);

    if std::env::var("MOCK_S3").unwrap_or_default() != "true" {
        let resp = match HttpClient::new().get(&cleaned_url).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                log::error!("kie download failed status={}", r.status());
                return HttpResponse::InternalServerError().finish();
            }
            Err(e) => {
                log::error!("kie download request error: {}", e);
                return HttpResponse::InternalServerError().finish();
            }
        };

        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                log::error!("kie download bytes error: {}", e);
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
            log::error!("kie s3 upload failed: {}", e);
            return HttpResponse::InternalServerError().finish();
        }
    }

    // Обновляем БД (runtime query, чтобы сборка не зависела от наличия таблиц в DEV БД)
    let cleaned_url = if std::env::var("MOCK_S3").unwrap_or_default() == "true" {
        cleaned_url
    } else {
        build_public_url(&state.s3_public_base_url, &state.s3_bucket, &s3_key)
    };

    let _ = sqlx::query(
        "UPDATE uploads SET cleaned_s3_key = $1, cleaned_url = $2, status = 'ready' WHERE task_id = $3",
    )
    .bind(&s3_key)
    .bind(cleaned_url)
    .bind(&task_id)
    .execute(&state.pool)
    .await;

    log::info!("kie callback processed task_id={}", task_id);
    HttpResponse::Ok().body("OK")
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
    handle_watermark_callback(payload, state).await
}

#[utoipa::path(
    post,
    path = "/callback/api/watermark-callback",
    tag = "webhooks",
    request_body = CallbackPayload,
    responses(
        (status = 200, description = "Callback processed"),
        (status = 400, description = "Task failed or invalid payload"),
        (status = 500, description = "Server error")
    )
)]
#[post("/callback/api/watermark-callback")]
pub async fn watermark_callback_alias(
    payload: web::Json<CallbackPayload>,
    state: web::Data<AppState>,
) -> HttpResponse {
    handle_watermark_callback(payload, state).await
}
