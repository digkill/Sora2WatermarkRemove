// src/api/webhooks.rs
use actix_web::{post, web, HttpResponse, Responder};
use serde::Deserialize;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{Client as S3Client, primitives::ByteStream};
use reqwest::Client as HttpClient;
use tokio::fs; // только для создания папки, если нужно логировать локально

#[derive(Deserialize)]
struct CallbackPayload {
    code: i32,
    msg: String,
    data: CallbackData,
}

#[derive(Deserialize)]
struct CallbackData {
    #[serde(rename = "taskId")]
    task_id: String,
    status: String,
    #[serde(rename = "outputUrl", default)]
    output_url: Option<String>,
}

// Рекомендую вынести S3 клиент в app_data для переиспользования
// В main.rs: .app_data(web::Data::new(s3_client))
#[post("/api/watermark-callback")]
pub async fn watermark_callback(
    payload: web::Json<CallbackPayload>,
    s3_client: web::Data<S3Client>,  // Внедряем через app_data
) -> impl Responder {
    let payload = payload.into_inner();

    // Проверяем успешность задачи
    if payload.code != 200 || payload.data.status != "success" {
        eprintln!(
            "Callback ошибка: code={}, msg={}, status={}",
            payload.code, payload.msg, payload.data.status
        );
        return HttpResponse::BadRequest().body("Task failed");
    }

    let cleaned_url = match payload.data.output_url {
        Some(url) => url,
        None => {
            eprintln!("outputUrl не найден в успешном callback");
            return HttpResponse::BadRequest().body("No output URL");
        }
    };

    let task_id = &payload.data.task_id;

    println!("Видео без водяного знака готово: {}", cleaned_url);
    println!("Task ID: {}", task_id);

    // Настройки S3 — лучше вынести в конфиг/env
    let bucket_name = std::env::var("S3_BUCKET").expect("S3_BUCKET must be set");
    let s3_key = format!("cleaned/{}.mp4", task_id); // можно добавить папки по дате и т.д.

    // Скачиваем видео как поток (не грузим целиком в память)
    let http_client = HttpClient::new();
    let response = match http_client.get(&cleaned_url).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                eprintln!("Ошибка скачивания видео: {}", resp.status());
                return HttpResponse::InternalServerError().body("Download failed");
            }
            resp
        }
        Err(e) => {
            eprintln!("Ошибка запроса к cleaned_url: {}", e);
            return HttpResponse::InternalServerError().body("Request failed");
        }
    };

    // Преобразуем в ByteStream для S3
    let byte_stream = ByteStream::from_response(response);

    // Загружаем в S3
    match s3_client
        .put_object()
        .bucket(&bucket_name)
        .key(&s3_key)
        .content_type("video/mp4")
        .body(byte_stream)
        .send()
        .await
    {
        Ok(_) => {
            println!("Видео успешно загружено в S3: s3://{}/{}", bucket_name, s3_key);

            // Здесь можно:
            // - Сохранить в БД: task_id → s3_url
            // - Отправить уведомление пользователю
            // - Сделать публичный URL: https://{bucket}.s3.amazonaws.com/{key}

            let public_url = format!("https://{}.s3.amazonaws.com/{}", bucket_name, s3_key);
            println!("Публичная ссылка: {}", public_url);
        }
        Err(e) => {
            eprintln!("Ошибка загрузки в S3: {}", e);
            return HttpResponse::InternalServerError().body("S3 upload failed");
        }
    }

    // Обязательно отвечаем 200 OK
    HttpResponse::Ok().body("OK")
}

#[post("/webhook/lava")]
pub async fn lava_webhook(payload: web::Json<serde_json::Value>) -> HttpResponse {
    println!("Webhook от lava.top: {:?}", payload);

    if let Some(status) = payload.get("status") {
        if status == "paid" {
            // TODO: запустить удаление водяного знака
            // Например: запускать асинх задачу remove_watermark()
        }
    }

    HttpResponse::Ok().finish()
}

