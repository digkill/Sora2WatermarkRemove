// src/models.rs

use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Product {
    pub id: i32,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub price: String,
    pub currency: String,
    pub product_type: String, // one_time | subscription
    pub credits_granted: Option<i32>,
    pub monthly_credits: Option<i32>,
    pub is_active: bool,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct Subscription {
    pub id: i32,
    pub user_id: i32,
    pub product_id: i32,
    pub provider: String,
    pub provider_subscription_id: Option<String>,
    pub status: String,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub canceled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct Transaction {
    pub id: i32,
    pub user_id: i32,
    pub product_id: Option<i32>,
    pub subscription_id: Option<i32>,
    pub provider: String,
    pub provider_order_id: String,
    pub amount: String,
    pub currency: String,
    pub status: String,  // pending | succeeded | failed | refunded
    pub tx_type: String, // payment | refund
    pub payload: Option<serde_json::Value>,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
}
