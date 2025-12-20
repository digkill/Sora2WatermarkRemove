use futures_util::StreamExt;
use lapin::{
    BasicProperties, Channel, Connection, ConnectionProperties,
    options::{BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions},
    types::FieldTable,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
struct TaskMessage {
    task_id: String,
}

#[derive(Debug, Deserialize)]
struct KieRecordResponse {
    code: i32,
    data: Option<KieRecordData>,
}

#[derive(Debug, Deserialize)]
struct KieRecordData {
    #[serde(rename = "taskId")]
    task_id: String,
    state: Option<String>,
    #[serde(rename = "resultJson")]
    result_json: Option<String>,
}

const QUEUE_NAME: &str = "kie.status.check";

pub async fn start_kie_status_queue(pool: PgPool, kie_api_key: String) {
    let rabbit_url = match std::env::var("RABBITMQ_URL") {
        Ok(url) => url,
        Err(_) => {
            log::warn!("RABBITMQ_URL not set, skipping status queue");
            return;
        }
    };

    let conn = match Connection::connect(&rabbit_url, ConnectionProperties::default()).await {
        Ok(c) => c,
        Err(e) => {
            log::error!("rabbitmq connect error: {e}");
            return;
        }
    };

    let channel = match conn.create_channel().await {
        Ok(c) => c,
        Err(e) => {
            log::error!("rabbitmq channel error: {e}");
            return;
        }
    };

    if let Err(e) = channel
        .queue_declare(QUEUE_NAME, QueueDeclareOptions::default(), FieldTable::default())
        .await
    {
        log::error!("rabbitmq declare queue error: {e}");
        return;
    }

    let poll_interval = std::env::var("KIE_STATUS_POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60);
    let batch_size = std::env::var("KIE_STATUS_BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50);

    let producer_pool = pool.clone();
    let producer_channel = channel.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = enqueue_pending_tasks(&producer_pool, &producer_channel, batch_size).await
            {
                log::error!("queue enqueue error: {e}");
            }
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
        }
    });

    let consumer_pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = consume_tasks(&consumer_pool, &channel, &kie_api_key).await {
            log::error!("queue consume error: {e}");
        }
    });
}

async fn enqueue_pending_tasks(
    pool: &PgPool,
    channel: &Channel,
    batch_size: i64,
) -> Result<(), String> {
    let rows = sqlx::query(
        r#"SELECT task_id
           FROM uploads
           WHERE status = 'processing'
             AND task_id IS NOT NULL
           ORDER BY created_at ASC
           LIMIT $1"#,
    )
    .bind(batch_size)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    for row in rows {
        let task_id: String = row.get("task_id");
        let payload = serde_json::to_vec(&TaskMessage { task_id }).map_err(|e| e.to_string())?;
        channel
            .basic_publish(
                "",
                QUEUE_NAME,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default(),
            )
            .await
            .map_err(|e| e.to_string())?
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

async fn consume_tasks(
    pool: &PgPool,
    channel: &Channel,
    kie_api_key: &str,
) -> Result<(), String> {
    let mut consumer = channel
        .basic_consume(
            QUEUE_NAME,
            "kie-status-consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await
        .map_err(|e| e.to_string())?;

    while let Some(delivery) = consumer.next().await {
        let delivery = match delivery {
            Ok(d) => d,
            Err(e) => {
                log::error!("rabbitmq delivery error: {e}");
                continue;
            }
        };

        if let Err(e) = handle_task_message(pool, &delivery.data, kie_api_key).await {
            log::error!("handle task message error: {e}");
        }

        let _ = delivery.ack(BasicAckOptions::default()).await;
    }

    Ok(())
}

async fn handle_task_message(pool: &PgPool, data: &[u8], kie_api_key: &str) -> Result<(), String> {
    let msg: TaskMessage = serde_json::from_slice(data).map_err(|e| e.to_string())?;
    let status = fetch_kie_status(&msg.task_id, kie_api_key).await?;

    match status.state.as_deref() {
        Some("success") => {
            if let Some(url) = status.result_url {
                sqlx::query(
                    r#"UPDATE uploads
                       SET status = 'ready', cleaned_url = $1
                       WHERE task_id = $2 AND status != 'ready'"#,
                )
                .bind(url)
                .bind(&msg.task_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            }
        }
        Some("fail") => {
            sqlx::query(
                r#"UPDATE uploads
                   SET status = 'failed'
                   WHERE task_id = $1 AND status != 'failed'"#,
            )
            .bind(&msg.task_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        _ => {}
    }

    Ok(())
}

struct KieStatus {
    state: Option<String>,
    result_url: Option<String>,
}

async fn fetch_kie_status(task_id: &str, kie_api_key: &str) -> Result<KieStatus, String> {
    let base_url =
        std::env::var("KIE_API_BASE_URL").unwrap_or_else(|_| "https://api.kie.ai".to_string());
    let url = format!("{base_url}/api/v1/jobs/recordInfo?taskId={task_id}");
    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", kie_api_key))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("kie status error status={} body={text}", status));
    }

    let parsed: KieRecordResponse =
        serde_json::from_str(&text).map_err(|e| format!("parse error: {e}; body={text}"))?;
    if parsed.code != 200 {
        return Err(format!("kie status code={} body={text}", parsed.code));
    }

    let data = parsed.data.ok_or_else(|| "missing data".to_string())?;
    let result_url = data
        .result_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .and_then(|value| value.get("resultUrls").and_then(|v| v.as_array()).cloned())
        .and_then(|arr| arr.first().and_then(|v| v.as_str()).map(|s| s.to_string()));

    Ok(KieStatus {
        state: data.state,
        result_url,
    })
}
