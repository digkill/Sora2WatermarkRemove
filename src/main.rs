// src/main.rs
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client as S3Client;
use dotenvy::dotenv;
use sqlx::PgPool;
use std::env;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use sora_watermark_remov::{AppState, api, docs};

async fn index() -> impl Responder {
    HttpResponse::Ok().body("Service ready!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to DB");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let kie_api_key = env::var("KIE_API_KEY").expect("KIE_API_KEY required");
    let s3_bucket = env::var("S3_BUCKET").expect("S3_BUCKET required");
    let s3_endpoint = env::var("S3_ENDPOINT").ok();
    let s3_public_base_url = env::var("S3_PUBLIC_BASE_URL")
        .unwrap_or_else(|_| format!("https://{}.s3.amazonaws.com", s3_bucket));
    let callback_base_url =
        env::var("CALLBACK_BASE_URL").unwrap_or_else(|_| "https://your-domain.com".to_string());

    let lava_api_key = env::var("LAVA_API_KEY").expect("LAVA_API_KEY required");
    let lava_webhook_key = env::var("LAVA_WEBHOOK_KEY").expect("LAVA_WEBHOOK_KEY required");

    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let mut s3_config_builder = aws_sdk_s3::config::Builder::from(&aws_config);

    // Allow custom S3-compatible endpoints (e.g., Beget, MinIO)
    if let Some(endpoint) = s3_endpoint {
        s3_config_builder = s3_config_builder
            .endpoint_url(endpoint)
            .force_path_style(true);
    }

    let s3_client = S3Client::from_conf(s3_config_builder.build());

    let state = web::Data::new(AppState {
        pool,
        s3_client,
        s3_bucket: s3_bucket.clone(),
        s3_public_base_url: s3_public_base_url.clone(),
        kie_api_key,
        callback_base_url,
        lava_api_key,
        lava_webhook_key,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(index))
            .service(
                SwaggerUi::new("/docs/{_:.*}")
                    .url("/api-docs/openapi.json", docs::ApiDoc::openapi()),
            )
            // Публичные роуты авторизации
            .service(api::auth::register)
            .service(api::auth::login)
            // Защищённые роуты
            .service(
                web::scope("/api")
                    .wrap(api::auth::JwtMiddleware)
                    .service(api::handlers::upload)
                    .service(api::products::list_products)
                    .service(api::payments::create_payment)
                    .service(api::subscriptions::list_subscriptions)
                    .service(api::subscriptions::cancel_subscription),
            )
            // Вебхуки (публичные)
            .service(api::webhooks::watermark_callback)
            .service(api::webhooks_lava::lava_webhook)
    })
    .bind(("0.0.0.0", 8065))?
    .run()
    .await
}
