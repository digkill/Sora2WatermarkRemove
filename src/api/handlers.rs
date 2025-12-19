// src/api/handlers.rs

use actix_multipart::Multipart;
use actix_web::{post, web, HttpResponse, Responder};
use aws_sdk_s3::primitives::ByteStream;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::billing::{can_remove_watermark, consume_credit};
use crate::AppState; // AppState в main.rs
use actix_web::web::ReqData;
use sqlx::Row;

#[post("/upload")]
pub async fn upload(
    mut payload: Multipart,
    state: web::Data<AppState>,
    user_id: ReqData<i32>, // получаем user_id из middleware
) -> impl Responder {
    let user_id = user_id.into_inner();

    // Проверка кредитов
    let credit_type = match can_remove_watermark(&state.pool, user_id).await {
        Ok(Some(t)) => t,
        Ok(None) => return HttpResponse::PaymentRequired().json(json!({
            "error": "Insufficient credits"
        })),
        Err(e) => {
            eprintln!("Billing error: {}", e);
            return HttpResponse::InternalServerError().body("Billing error");
        }
    };

    // Чтение файла
    let mut file_bytes: Vec<u8> = Vec::new();
    let mut original_filename = "video.mp4".to_string();

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(_) => continue,
        };

        // Получаем имя файла (actix-multipart 0.6: content_disposition() -> &ContentDisposition)
        let cd = field.content_disposition();
        if let Some(name) = cd.get_filename() {
            original_filename = sanitize(name);
        }

        // Читаем чанки
        while let Some(chunk) = field.next().await {
            if let Ok(data) = chunk {
                file_bytes.extend_from_slice(&data);
            }
        }
    }

    if file_bytes.is_empty() {
        return HttpResponse::BadRequest().body("No file uploaded");
    }

    // Загрузка в S3
    let original_key = format!("original/{}/{}.mp4", user_id, Uuid::new_v4());
    let stream = ByteStream::from(file_bytes);

    if let Err(e) = state.s3_client
        .put_object()
        .bucket(&state.s3_bucket)
        .key(&original_key)
        .content_type("video/mp4")
        .body(stream)
        .send()
        .await
    {
        eprintln!("S3 error: {}", e);
        return HttpResponse::InternalServerError().body("Failed to save to S3");
    }

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
            eprintln!("DB insert error: {}", e);
            return HttpResponse::InternalServerError().body("DB error");
        }
    };

    // Списываем кредит
    let _ = consume_credit(&state.pool, user_id, &credit_type).await;

    // Запуск Kie.ai
    let public_url = format!("https://{}.s3.amazonaws.com/{}", state.s3_bucket, original_key);
    let callback_url = format!("{}/api/watermark-callback", state.callback_base_url);

    match start_remove_watermark(&state.kie_api_key, &public_url, &callback_url).await {
        Ok(task_id) => {
            let _ = sqlx::query("UPDATE uploads SET task_id = $1 WHERE id = $2")
                .bind(&task_id)
                .bind(upload_id)
                .execute(&state.pool)
                .await;

            HttpResponse::Ok().json(json!({
                "message": "Processing started",
                "upload_id": upload_id,
                "task_id": task_id
            }))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e })),
    }
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
    let body = json!({
        "model": "sora-watermark-remover",
        "input": { "video_url": video_url },
        "callBackUrl": callback_url
    });

    let resp = client
        .post("https://api.kie.ai/api/v1/jobs/createTask")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let json: Value = resp.json().await.map_err(|e| e.to_string())?;
    json["data"]["taskId"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No taskId in response".to_string())
}
