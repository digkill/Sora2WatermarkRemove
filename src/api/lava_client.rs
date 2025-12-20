// src/api/lava_client.rs
//
// Минимальный клиент для lava.top Public API (https://gate.lava.top)
// Авторизация: заголовок X-Api-Key

use serde::{Deserialize, Serialize};
use std::fmt;

const LAVA_API_BASE: &str = "https://gate.lava.top";

#[derive(Debug)]
pub enum LavaError {
    Http(reqwest::Error),
    Api { status: u16, body: String },
    InvalidResponse(String),
}

impl fmt::Display for LavaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LavaError::Http(e) => write!(f, "http error: {e}"),
            LavaError::Api { status, body } => {
                write!(f, "lava api error status={status} body={body}")
            }
            LavaError::InvalidResponse(e) => write!(f, "invalid response: {e}"),
        }
    }
}

impl From<reqwest::Error> for LavaError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

#[derive(Debug, Serialize)]
pub struct CreateInvoiceV3Request {
    pub email: String,
    #[serde(rename = "offerId")]
    pub offer_id: String,
    pub currency: String,

    #[serde(rename = "paymentProvider", skip_serializing_if = "Option::is_none")]
    pub payment_provider: Option<String>,

    #[serde(rename = "paymentMethod", skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub periodicity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InvoicePaymentParamsResponse {
    pub id: String,
    pub status: String,

    #[serde(rename = "paymentUrl")]
    pub payment_url: Option<String>,
}

pub async fn create_invoice_v3(
    lava_api_key: &str,
    req: CreateInvoiceV3Request,
) -> Result<InvoicePaymentParamsResponse, LavaError> {
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{LAVA_API_BASE}/api/v3/invoice"))
        .header("X-Api-Key", lava_api_key)
        .json(&req)
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        return Err(LavaError::Api {
            status: status.as_u16(),
            body,
        });
    }

    serde_json::from_str::<InvoicePaymentParamsResponse>(&body)
        .map_err(|e| LavaError::InvalidResponse(format!("{e}; body={body}")))
}

pub async fn cancel_subscription(
    lava_api_key: &str,
    parent_contract_id: &str,
    buyer_email: &str,
) -> Result<(), LavaError> {
    let client = reqwest::Client::new();

    let resp = client
        .delete(format!("{LAVA_API_BASE}/api/v1/subscriptions"))
        .header("X-Api-Key", lava_api_key)
        .query(&[("contractId", parent_contract_id), ("email", buyer_email)])
        .send()
        .await?;

    if resp.status().as_u16() == 204 {
        return Ok(());
    }

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(LavaError::Api {
            status: status.as_u16(),
            body,
        });
    }

    Ok(())
}

/// Маппинг нашего `product_slug` -> Lava `offerId`.
///
/// ВАЖНО: заполните реальными UUID из lava.top (offerId) для ваших тарифов.
///
/// Порядок действий:
/// - получить список продуктов/офферов в lava.top
/// - для нужного оффера скопировать UUID
/// - вставить сюда
pub fn offer_id_for_product_slug(product_slug: &str) -> Option<&'static str> {
    match product_slug {
        // примеры:
        // "sub_basic" => Some("836b9fc5-7ae9-4a27-9642-592bc44072b7"),
        _ => None,
    }
}
