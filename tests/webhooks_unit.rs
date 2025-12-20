use actix_web::test::TestRequest;
use serde_json::json;

use sora_watermark_remov::api::webhooks_lava::{
    extract_api_key,
    normalize_payload,
    parse_webhook_body,
};

#[test]
fn normalize_payment_success_example() {
    let raw = json!({
        "eventType": "payment.success",
        "product": {
            "id": "d31384b8-e412-4be5-a2ec-297ae6666c8f",
            "title": "Тестовый продукт"
        },
        "buyer": {
            "email": "test@lava.top"
        },
        "contractId": "7ea82675-4ded-4133-95a7-a6efbaf165cc",
        "amount": 40254.19,
        "currency": "RUB",
        "timestamp": "2024-02-05T09:38:27.33277Z",
        "status": "completed",
        "errorMessage": ""
    });

    let normalized = normalize_payload(raw);
    assert_eq!(normalized.event_type.as_deref(), Some("payment.success"));
    assert_eq!(
        normalized.order_id.as_deref(),
        Some("7ea82675-4ded-4133-95a7-a6efbaf165cc")
    );
    assert_eq!(
        normalized.product_offer_id.as_deref(),
        Some("d31384b8-e412-4be5-a2ec-297ae6666c8f")
    );
    assert_eq!(normalized.buyer_email.as_deref(), Some("test@lava.top"));
    assert_eq!(normalized.status.as_deref(), Some("completed"));
    assert_eq!(normalized.currency.as_deref(), Some("RUB"));
    assert_eq!(normalized.amount.as_deref(), Some("40254.19"));
}

#[test]
fn parse_form_payload() {
    let body = b"contractId=abc&status=completed&eventType=payment.success";
    let raw = parse_webhook_body(body).expect("parse form");
    let normalized = normalize_payload(raw);

    assert_eq!(normalized.order_id.as_deref(), Some("abc"));
    assert_eq!(normalized.status.as_deref(), Some("completed"));
    assert_eq!(normalized.event_type.as_deref(), Some("payment.success"));
}

#[test]
fn extract_api_key_from_header() {
    let req = TestRequest::default()
        .insert_header(("X-Api-Key", "secret"))
        .to_http_request();
    let payload = json!({});
    let key = extract_api_key(&req, &payload);
    assert_eq!(key.as_deref(), Some("secret"));
}
