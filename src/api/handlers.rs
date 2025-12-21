// src/api/handlers.rs

use actix_multipart::Multipart;
use actix_web::{HttpResponse, Responder, get, post, web};
use futures_util::StreamExt;
use reqwest::Client;
use serde::Serialize;
use serde_json::{Value, json};
use utoipa::ToSchema;

use crate::AppState; // AppState в main.rs
use crate::billing::{can_remove_watermark, consume_credit};
use actix_web::web::ReqData;
use sqlx::Row;

#[derive(ToSchema)]
pub struct UrlUploadBody {
    /// Public video URL (Sora share link or direct MP4 URL)
    pub url: String,
}

#[derive(Serialize, ToSchema)]
pub struct UploadResponse {
    pub message: String,
    pub upload_id: i32,
    pub task_id: String,
}

#[derive(Serialize, ToSchema)]
pub struct CreditsStatusResponse {
    pub credits: i32,
    pub monthly_quota: i32,
    pub free_generation_used: bool,
}

#[derive(Serialize, ToSchema)]
pub struct UploadItemResponse {
    pub id: i32,
    pub status: String,
    pub original_filename: String,
    pub cleaned_url: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[utoipa::path(
    post,
    path = "/api/upload",
    tag = "uploads",
    request_body(
        content = UrlUploadBody,
        content_type = "multipart/form-data"
    ),
    responses(
        (status = 200, description = "Upload accepted", body = UploadResponse),
        (status = 401, description = "Unauthorized"),
        (status = 402, description = "Insufficient credits"),
        (status = 500, description = "Server error")
    )
)]
#[post("/upload")]
pub async fn upload(
    mut payload: Multipart,
    state: web::Data<AppState>,
    user_id: ReqData<i32>, // получаем user_id из middleware
) -> impl Responder {
    let user_id = user_id.into_inner();
    log::info!("upload start user_id={}", user_id);

    // Проверка кредитов
    let credit_type = match can_remove_watermark(&state.pool, user_id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            log::warn!("upload no credits user_id={}", user_id);
            return HttpResponse::PaymentRequired().json(json!({
                "error": "Insufficient credits"
            }));
        }
        Err(e) => {
            log::error!("upload billing error user_id={} error={}", user_id, e);
            return HttpResponse::InternalServerError().body("Billing error");
        }
    };

    // Чтение формы (только URL)
    let mut original_filename = "video.mp4".to_string();
    let mut url_value: Option<String> = None;
    let mut file_provided = false;

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(_) => continue,
        };

        let field_name = field.name();
        if field_name == "url" {
            let mut url_bytes: Vec<u8> = Vec::new();
            while let Some(chunk) = field.next().await {
                if let Ok(data) = chunk {
                    url_bytes.extend_from_slice(&data);
                }
            }
            if let Ok(url_str) = String::from_utf8(url_bytes) {
                let trimmed = url_str.trim();
                if !trimmed.is_empty() {
                    url_value = Some(trimmed.to_string());
                }
            }
            continue;
        }

        // Файловые загрузки временно отключены
        file_provided = true;
        while let Some(chunk) = field.next().await {
            let _ = chunk;
        }
    }

    if file_provided {
        return HttpResponse::BadRequest().body("File uploads are disabled. Please provide a video URL.");
    }

    let external_url: Option<String> = if let Some(url) = url_value.as_deref() {
        log::info!("upload using external url user_id={} url={}", user_id, url);
        if let Some(name) = url.rsplit('/').next() {
            let name = name.split('?').next().unwrap_or(name);
            let name = name.split('#').next().unwrap_or(name);
            if !name.is_empty() {
                original_filename = sanitize(name);
            }
        }
        Some(url.to_string())
    } else {
        return HttpResponse::BadRequest().body("Video URL is required");
    };

    // Сохраняем URL как источник
    let original_key = external_url.clone().unwrap_or_default();

    // Вставка в БД (runtime query, чтобы сборка не зависела от наличия таблиц в DEV БД)
    let upload_id: i32 = match sqlx::query(
        "INSERT INTO uploads (user_id, original_filename, original_s3_key, status, used_credit_type)\n         VALUES ($1, $2, $3, 'processing', $4) RETURNING id",
    )
    .bind(user_id)
    .bind(&original_filename)
    .bind(&original_key)
    .bind(&credit_type)
    .fetch_one(&state.pool)
    .await
    {
        Ok(row) => row.get("id"),
        Err(e) => {
            log::error!("upload db insert error user_id={} error={}", user_id, e);
            return HttpResponse::InternalServerError().body("DB error");
        }
    };

    // Списываем кредит
    let _ = consume_credit(&state.pool, user_id, &credit_type).await;

    // Запуск Kie.ai
    let public_url = external_url.unwrap_or_default();
    let callback_url = format!("{}/api/watermark-callback", state.callback_base_url);

    match start_remove_watermark(&state.kie_api_key, &public_url, &callback_url).await {
        Ok(task_id) => {
            log::info!("upload started task user_id={} upload_id={} task_id={}", user_id, upload_id, task_id);
            let _ = sqlx::query("UPDATE uploads SET task_id = $1 WHERE id = $2")
                .bind(&task_id)
                .bind(upload_id)
                .execute(&state.pool)
                .await;

            let response = UploadResponse {
                message: "Processing started".to_string(),
                upload_id,
                task_id,
            };

            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            log::error!("upload kie error user_id={} upload_id={} error={}", user_id, upload_id, e);
            HttpResponse::InternalServerError().json(json!({ "error": e }))
        }
    }
}

#[get("/credits")]
pub async fn credits_status(
    state: web::Data<AppState>,
    user_id: ReqData<i32>,
) -> impl Responder {
    let user_id = user_id.into_inner();

    let row = match sqlx::query(
        "SELECT credits, monthly_quota, free_generation_used FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("credits_status db error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let Some(row) = row else {
        return HttpResponse::NotFound().finish();
    };

    HttpResponse::Ok().json(CreditsStatusResponse {
        credits: row.get("credits"),
        monthly_quota: row.get("monthly_quota"),
        free_generation_used: row.get("free_generation_used"),
    })
}

#[get("/uploads")]
pub async fn list_uploads(
    state: web::Data<AppState>,
    user_id: ReqData<i32>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let user_id = user_id.into_inner();
    let limit: i64 = query
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(100)
        .clamp(1, 1000);
    let offset: i64 = query
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);

    let rows = match sqlx::query(
        r#"SELECT id, status, original_filename, cleaned_url, created_at
           FROM uploads
           WHERE user_id = $1
           ORDER BY created_at DESC
           LIMIT $2 OFFSET $3"#,
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            log::error!("list_uploads db error user_id={} error={}", user_id, e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let items: Vec<UploadItemResponse> = rows
        .into_iter()
        .map(|row| UploadItemResponse {
            id: row.get("id"),
            status: row.get("status"),
            original_filename: row.get("original_filename"),
            cleaned_url: row.get("cleaned_url"),
            created_at: row.get("created_at"),
        })
        .collect();

    HttpResponse::Ok().json(items)
}

// Санитизация имени файла
fn sanitize(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
        .collect()
}

// Запуск задачи в Kie.ai
pub async fn start_remove_watermark(
    api_key: &str,
    video_url: &str,
    callback_url: &str,
) -> Result<String, String> {
    let client = Client::new();
    let base_url = std::env::var("KIE_API_BASE_URL").unwrap_or_else(|_| "https://api.kie.ai".to_string());
    let body = json!({
        "model": "sora-watermark-remover",
        "input": { "video_url": video_url },
        "callBackUrl": callback_url
    });

    let resp = client
        .post(format!("{base_url}/api/v1/jobs/createTask"))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("KIE error status={} body={}", status, text));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("error decoding response body: {e}; body={text}"))?;
    json["data"]["taskId"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("No taskId in response; body={text}"))
}
