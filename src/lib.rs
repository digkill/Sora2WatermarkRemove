pub mod api;
pub mod billing;
pub mod db;
pub mod docs;
pub mod models;
pub mod s3_utils;

use aws_sdk_s3::Client as S3Client;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub s3_client: S3Client,
    pub s3_bucket: String,
    pub s3_public_base_url: String,
    pub kie_api_key: String,
    pub callback_base_url: String,
    pub lava_api_key: String,
    pub lava_webhook_key: String,
}
