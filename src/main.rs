// src/main.rs

mod api;
mod billing;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use api::handlers::{upload}; // наши хендлеры
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client as S3Client;
use dotenv::dotenv;
use std::env;

// Структура конфига (расширим при необходимости)
#[derive(Clone)]
pub struct Config {
    pub kie_api_key: String,
    pub lava_api_key: String,
    // Можно добавить: s3_bucket, domain и т.д.
}

async fn index() -> impl Responder {
    HttpResponse::Ok().body("Service ready!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Загружаем .env файл
    dotenv().ok();

    // Обязательные переменные
    let kie_api_key = env::var("KIE_API_KEY").expect("KIE_API_KEY must be set");
    let lava_api_key = env::var("LAVA_API_KEY").expect("LAVA_API_KEY must be set");
    let s3_bucket = env::var("S3_BUCKET").expect("S3_BUCKET must be set");

    // Опционально: базовый URL для callback (например, ваш домен)
    // Если не задан — попробуем собрать из REQUEST_SCHEME/HOST, но лучше задать явно
    let callback_base_url = env::var("CALLBACK_BASE_URL")
        .unwrap_or_else(|_| "https://your-domain.com".to_string());

    // Инициализируем AWS S3 клиент
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let aws_config = aws_config::from_env().region(region_provider).load().await;
    let s3_client = S3Client::new(&aws_config);

    // Упаковываем конфиг и клиенты в app_data
    let app_config = web::Data::new(Config {
        kie_api_key,
        lava_api_key,
    });

    let s3_client_data = web::Data::new(s3_client);
    let s3_bucket_data = web::Data::new(s3_bucket);
    let callback_base_data = web::Data::new(callback_base_url);

    println!("Сервер запускается на порту 8065...");
    println!("Callback URL будет: {}/api/watermark-callback", callback_base_data.get_ref());

    HttpServer::new(move || {
        App::new()
            // Глобальные данные
            .app_data(app_config.clone())
            .app_data(s3_client_data.clone())
            .app_data(s3_bucket_data.clone())
            .app_data(callback_base_data.clone())

            // Маршруты
            .route("/", web::get().to(index))

            // Ваши существующие платежные вебхуки и сервисы
            .service(api::payments::create_lava_payment)
            .service(api::webhooks::lava_webhook)

            // Новый маршрут загрузки видео
            .service(upload)

            // Webhook от Kie.ai — сюда придёт результат обработки
            .service(api::webhooks::watermark_callback)
    })
        .bind(("0.0.0.0", 8065))?
        .run()
        .await
}