// src/main.rs
mod api;
mod billing;
mod db;
mod models;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use aws_sdk_s3::Client as S3Client;
use aws_config::meta::region::RegionProviderChain;
use dotenvy::dotenv;
use sqlx::PgPool;
use std::env;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub s3_client: S3Client,
    pub s3_bucket: String,
    pub kie_api_key: String,
    pub callback_base_url: String,

    // Lava.top
    pub lava_project_id: String,
    pub lava_secret_key: String,
}

async fn index() -> impl Responder {
    HttpResponse::Ok().body("Service ready!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url).await.expect("Failed to connect to DB");

    sqlx::migrate!().run(&pool).await.expect("Failed to run migrations");

    let kie_api_key = env::var("KIE_API_KEY").expect("KIE_API_KEY required");
    let s3_bucket = env::var("S3_BUCKET").expect("S3_BUCKET required");
    let callback_base_url = env::var("CALLBACK_BASE_URL")
        .unwrap_or_else(|_| "https://your-domain.com".to_string());

    let lava_project_id = env::var("LAVA_PROJECT_ID").expect("LAVA_PROJECT_ID required");
    let lava_secret_key = env::var("LAVA_SECRET_KEY").expect("LAVA_SECRET_KEY required");

    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let s3_client = S3Client::new(&aws_config);

    let state = web::Data::new(AppState {
        pool,
        s3_client,
        s3_bucket: s3_bucket.clone(),
        kie_api_key,
        callback_base_url,
        lava_project_id,
        lava_secret_key,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(index))

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
                    .service(api::subscriptions::cancel_subscription)
            )

            // Вебхуки (публичные)
            .service(api::webhooks::watermark_callback)
            .service(api::webhooks_lava::lava_webhook)
    })
        .bind(("0.0.0.0", 8065))?
        .run()
        .await
}
