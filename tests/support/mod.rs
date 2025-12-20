use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::Client as S3Client;
use sqlx::PgPool;
use std::env;
use std::sync::OnceLock;
use tokio::sync::{Mutex, MutexGuard};

use sora_watermark_remov::AppState;

fn split_db_url(url: &str) -> Result<(String, String), String> {
    let (base, query) = match url.split_once('?') {
        Some((base, query)) => (base.to_string(), Some(query)),
        None => (url.to_string(), None),
    };

    let db_start = base
        .rfind('/')
        .ok_or_else(|| "invalid database url".to_string())?;
    if db_start + 1 >= base.len() {
        return Err("database name is empty".to_string());
    }

    let db_name = base[db_start + 1..].to_string();
    let mut admin_url = format!("{}postgres", &base[..db_start + 1]);
    if let Some(query) = query {
        admin_url = format!("{admin_url}?{query}");
    }

    Ok((admin_url, db_name))
}

fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

static TEST_DB_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub struct TestDb {
    pub pool: PgPool,
    _guard: MutexGuard<'static, ()>,
}

pub async fn init_test_db() -> TestDb {
    dotenvy::dotenv().ok();
    let test_url = env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL must be set");
    let (admin_url, db_name) =
        split_db_url(&test_url).expect("invalid TEST_DATABASE_URL format");

    let lock = TEST_DB_LOCK.get_or_init(|| Mutex::new(()));
    let guard = lock.lock().await;

    let admin_pool = PgPool::connect(&admin_url)
        .await
        .expect("connect admin db");

    let _ = sqlx::query("SELECT pg_advisory_lock(424242)")
        .execute(&admin_pool)
        .await;

    let quoted_name = quote_identifier(&db_name);
    let drop_sql = format!("DROP DATABASE IF EXISTS {quoted_name} WITH (FORCE)");
    let create_sql = format!("CREATE DATABASE {quoted_name}");

    let _ = sqlx::query(&drop_sql).execute(&admin_pool).await;
    let create_result = sqlx::query(&create_sql).execute(&admin_pool).await;
    if let Err(e) = create_result {
        eprintln!("create test db error: {e}");
        let _ = sqlx::query(&drop_sql).execute(&admin_pool).await;
        sqlx::query(&create_sql)
            .execute(&admin_pool)
            .await
            .expect("create test db retry");
    }

    let _ = sqlx::query("SELECT pg_advisory_unlock(424242)")
        .execute(&admin_pool)
        .await;

    admin_pool.close().await;

    let pool = PgPool::connect(&test_url)
        .await
        .expect("connect test db");
    sqlx::migrate!().run(&pool).await.expect("migrations");
    TestDb { pool, _guard: guard }
}

pub async fn build_state(pool: PgPool, lava_webhook_key: &str) -> AppState {
    let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
    let aws_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region_provider)
        .load()
        .await;
    let s3_client = S3Client::from_conf(aws_sdk_s3::config::Builder::from(&aws_config).build());

    AppState {
        pool,
        s3_client,
        s3_bucket: "test-bucket".to_string(),
        s3_public_base_url: "http://localhost".to_string(),
        kie_api_key: "test-kie".to_string(),
        callback_base_url: "http://localhost".to_string(),
        lava_api_key: "test-lava".to_string(),
        lava_webhook_key: lava_webhook_key.to_string(),
    }
}
