use actix_multipart::Multipart;
use actix_web::{post, web, HttpResponse, Responder};
use futures_util::StreamExt;
use uuid::Uuid;
use crate::billing::{can_remove_watermark, consume_credit};
use crate::kie::start_remove_watermark;
use crate::main::AppState;

#[derive(Deserialize)]
struct CreateTaskResponse {
    code: i32,
    msg: String,
    data: CreateTaskData,
}

#[derive(Deserialize)]
struct CreateTaskData {
    #[serde(rename = "taskId")]
    task_id: String,
}

// Простая функция санитизации имени файла (чтобы избежать path traversal и т.п.)
fn sanitize(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
        .collect()
}

#[post("/api/upload")]
pub async fn upload(
    mut payload: Multipart,
    state: web::Data<AppState>,
    user: web::ReqData<i32>, // user_id из JWT middleware
) -> impl Responder {
    let user_id = *user;

    // Проверка доступных кредитов
    let credit_type = match can_remove_watermark(&state.pool, user_id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return HttpResponse::PaymentRequired().json(serde_json::json!({
                "error": "Insufficient credits",
                "message": "Buy a pack or subscribe to continue"
            }));
        }
        Err(e) => {
            eprintln!("DB error: {}", e);
            return HttpResponse::InternalServerError().body("Database error");
        }
    };

    // Обработка файла
    let mut original_filename = "video.mp4".to_string();
    let mut file_bytes = Vec::new();

    while let Some(item) = payload.next().await {
        let mut field = item.unwrap();
        if let Some(filename) = field.content_disposition().and_then(|d| d.get_filename()) {
            original_filename = filename.to_string();
        }

        while let Some(chunk) = field.next().await {
            file_bytes.extend_from_slice(&chunk.unwrap());
        }
    }

    if file_bytes.is_empty() {
        return HttpResponse::BadRequest().body("No file uploaded");
    }

    // Загружаем оригинал в S3
    let original_key = format!("original/{}/{}.mp4", user_id, Uuid::new_v4());
    let byte_stream = aws_sdk_s3::primitives::ByteStream::from(file_bytes);

    if let Err(e) = state.s3_client.put_object()
        .bucket(&state.s3_bucket)
        .key(&original_key)
        .content_type("video/mp4")
        .body(byte_stream)
        .send()
        .await
    {
        eprintln!("S3 upload error: {}", e);
        return HttpResponse::InternalServerError().body("Failed to save original");
    }

    // Создаём запись в БД
    let upload_record = sqlx::query!(
        "INSERT INTO uploads (user_id, original_filename, original_s3_key, status, used_credit_type)
         VALUES ($1, $2, $3, 'processing', $4) RETURNING id",
        user_id, original_filename, original_key, credit_type
    )
        .fetch_one(&state.pool)
        .await
        .map_err(|e| {
            eprintln!("DB insert error: {}", e);
            HttpResponse::InternalServerError().body("DB error")
        })?;

    // Списываем кредит
    consume_credit(&state.pool, user_id, &credit_type).await.ok();

    // Формируем публичный URL оригинала (для Kie.ai)
    let public_original_url = format!("https://{}.s3.amazonaws.com/{}", state.s3_bucket, original_key);

    // Формируем callback URL
    let callback_url = format!("{}/api/watermark-callback", state.callback_base_url);

    // Запускаем обработку
    match start_remove_watermark(&state.kie_api_key, &public_original_url, &callback_url).await {
        Ok(task_id) => {
            // Обновляем task_id
            sqlx::query!("UPDATE uploads SET task_id = $1, status = 'processing' WHERE id = $2", task_id, upload_record.id)
                .execute(&state.pool)
                .await
                .ok();

            HttpResponse::Ok().json(serde_json::json!({
                "message": "Video uploaded and processing started",
                "upload_id": upload_record.id,
                "task_id": task_id,
                "credit_used": credit_type
            }))
        }
        Err(e) => {
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to start processing",
                "details": e
            }))
        }
    }
}

/// Создаёт задачу на удаление водяного знака и возвращает task_id сразу
/// Kie.ai сам пришлёт результат на callback_url по завершении
pub async fn start_remove_watermark(
    api_key: &str,
    video_url: &str,
    callback_url: &str,  // Ваш публичный endpoint, например https://your-domain.com/api/watermark-callback
) -> Result<String, String> {
    let client = Client::new();

    let body = json!({
        "model": "sora-watermark-remover",
        "input": {
            "video_url": video_url
        },
        "callBackUrl": callback_url
    });

    let resp = client
        .post("https://api.kie.ai/api/v1/jobs/createTask")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ошибка отправки: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP ошибка: {}", resp.status()));
    }

    let json: CreateTaskResponse = resp
        .json()
        .await
        .map_err(|e| format!("Ошибка парсинга ответа: {}", e))?;

    if json.code != 200 {
        return Err(format!("API ошибка: {}", json.msg));
    }

    Ok(json.data.task_id)
}

