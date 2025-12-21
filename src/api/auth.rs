// src/api/auth.rs

use actix_web::Error;
use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{HttpMessage, HttpResponse, Responder, get, post, web};
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::{Duration, Utc};
use futures_util::future::{LocalBoxFuture, Ready, ready};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use lettre::message::{Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::task::{Context, Poll};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: i32,
    exp: usize,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub token: Option<String>,
    pub user_id: i32,
    pub verification_required: bool,
}

#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "User registered", body = AuthResponse),
        (status = 400, description = "User already exists or invalid data"),
        (status = 500, description = "Server error")
    )
)]
#[post("/auth/register")]
pub async fn register(
    state: web::Data<AppState>,
    payload: web::Json<RegisterRequest>,
) -> impl Responder {
    let password_hash = match hash(&payload.password, DEFAULT_COST) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("bcrypt hash error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let row = match sqlx::query(
        r#"INSERT INTO users (username, email, password_hash, credits)
           VALUES ($1, $2, $3, 0)
           RETURNING id"#,
    )
    .bind(payload.username.as_deref())
    .bind(&payload.email)
    .bind(password_hash)
    .fetch_one(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("register db error: {e}");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "user already exists or invalid data"
            }));
        }
    };

    let user_id: i32 = row.get("id");

    if let Err(e) = create_and_send_verification(&state.pool, user_id, &payload.email).await {
        eprintln!("send verification error: {e}");
    }

    HttpResponse::Ok().json(AuthResponse {
        token: None,
        user_id,
        verification_required: true,
    })
}

#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Authenticated", body = AuthResponse),
        (status = 401, description = "Invalid credentials"),
        (status = 500, description = "Server error")
    )
)]
#[post("/auth/login")]
pub async fn login(state: web::Data<AppState>, payload: web::Json<LoginRequest>) -> impl Responder {
    let row = match sqlx::query(
        r#"SELECT id, password_hash, email_verified
           FROM users
           WHERE email = $1"#,
    )
        .bind(&payload.email)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("login db error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let Some(row) = row else {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "invalid credentials"
        }));
    };

    let user_id: i32 = row.get("id");
    let password_hash: String = row.get("password_hash");
    let email_verified: bool = row.get("email_verified");

    match verify(&payload.password, &password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "invalid credentials"
            }));
        }
        Err(e) => {
            eprintln!("bcrypt verify error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    if !email_verified {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "error": "email not verified"
        }));
    }

    let token = match generate_jwt(user_id) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("jwt encode error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(AuthResponse {
        token: Some(token),
        user_id,
        verification_required: false,
    })
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResendVerificationRequest {
    pub email: String,
}

#[utoipa::path(
    post,
    path = "/auth/resend-verification",
    tag = "auth",
    request_body = ResendVerificationRequest,
    responses(
        (status = 200, description = "Verification email sent"),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Server error")
    )
)]
#[post("/auth/resend-verification")]
pub async fn resend_verification(
    state: web::Data<AppState>,
    payload: web::Json<ResendVerificationRequest>,
) -> impl Responder {
    let row = match sqlx::query(
        r#"SELECT id, email_verified
           FROM users
           WHERE email = $1"#,
    )
    .bind(&payload.email)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("resend verification db error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let Some(row) = row else {
        return HttpResponse::Ok().json(serde_json::json!({"ok": true}));
    };

    let user_id: i32 = row.get("id");
    let email_verified: bool = row.get("email_verified");
    if email_verified {
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "already_verified": true}));
    }

    if let Err(e) = create_and_send_verification(&state.pool, user_id, &payload.email).await {
        eprintln!("resend verification error: {e}");
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().json(serde_json::json!({"ok": true}))
}

#[utoipa::path(
    get,
    path = "/auth/verify",
    tag = "auth",
    params(
        ("token" = String, Query, description = "Verification token")
    ),
    responses(
        (status = 200, description = "Email verified"),
        (status = 400, description = "Invalid or expired token"),
        (status = 500, description = "Server error")
    )
)]
#[get("/auth/verify")]
pub async fn verify_email(
    state: web::Data<AppState>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> impl Responder {
    let Some(token_str) = query.get("token") else {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "missing token"}));
    };

    let token = match Uuid::parse_str(token_str) {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error": "invalid token"}));
        }
    };

    let row = match sqlx::query(
        r#"SELECT user_id, expires_at
           FROM email_verification_tokens
           WHERE token = $1"#,
    )
    .bind(token)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("verify email db error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let Some(row) = row else {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "invalid token"}));
    };

    let user_id: i32 = row.get("user_id");
    let expires_at: chrono::DateTime<Utc> = row.get("expires_at");
    if expires_at < Utc::now() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "token expired"}));
    }

    let updated = match sqlx::query(
        r#"UPDATE users
           SET email_verified = true, email_verified_at = NOW()
           WHERE id = $1 AND email_verified = false
           RETURNING email_verified"#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            eprintln!("verify email update error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    if updated.is_none() {
        return HttpResponse::Ok().json(serde_json::json!({"ok": true, "already_verified": true}));
    }

    let _ = sqlx::query("DELETE FROM email_verification_tokens WHERE token = $1")
        .bind(token)
        .execute(&state.pool)
        .await;

    HttpResponse::Ok().json(serde_json::json!({"ok": true}))
}

async fn create_and_send_verification(
    pool: &sqlx::PgPool,
    user_id: i32,
    email: &str,
) -> Result<(), String> {
    let token = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::hours(24);

    let _ = sqlx::query("DELETE FROM email_verification_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query(
        r#"INSERT INTO email_verification_tokens (token, user_id, expires_at)
           VALUES ($1, $2, $3)"#,
    )
    .bind(token)
    .bind(user_id)
    .bind(expires_at)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let app_base = std::env::var("APP_BASE_URL").map_err(|_| "APP_BASE_URL must be set".to_string())?;
    let verify_url = format!("{app_base}/verify?token={token}");

    let smtp_host = std::env::var("SMTP_HOST").ok();
    let smtp_user = std::env::var("SMTP_USER").ok();
    let smtp_pass = std::env::var("SMTP_PASS").ok();
    let smtp_from = std::env::var("SMTP_FROM").ok();
    let smtp_port = std::env::var("SMTP_PORT").ok().and_then(|p| p.parse::<u16>().ok());

    if smtp_host.is_none() || smtp_from.is_none() {
        eprintln!("SMTP not configured. Verification link: {verify_url}");
        return Ok(());
    }

    let from: Mailbox = smtp_from
        .clone()
        .unwrap()
        .parse::<Mailbox>()
        .map_err(|e| e.to_string())?;
    let to: Mailbox = email.parse::<Mailbox>().map_err(|e| e.to_string())?;

    let email_message = Message::builder()
        .from(from)
        .to(to)
        .subject("Confirm your email for Sora Clean")
        .body(format!(
            "Welcome to Sora Clean!\n\nPlease confirm your email by clicking the link:\n{verify_url}\n\nIf you did not request this, you can ignore this email."
        ))
        .map_err(|e| e.to_string())?;

    let smtp_port = smtp_port.unwrap_or(587);
    let smtp_security = std::env::var("SMTP_SECURITY").unwrap_or_else(|_| {
        if smtp_port == 465 {
            "ssl".to_string()
        } else {
            "starttls".to_string()
        }
    });

    let mut builder = match smtp_security.as_str() {
        "ssl" => AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host.clone().unwrap())
            .map_err(|e| e.to_string())?,
        "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
            smtp_host.clone().unwrap(),
        ),
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_host.clone().unwrap())
            .map_err(|e| e.to_string())?,
    };
    builder = builder.port(smtp_port);

    let mailer = if let (Some(user), Some(pass)) = (smtp_user, smtp_pass) {
        let creds = Credentials::new(user, pass);
        builder.credentials(creds).build()
    } else {
        builder.build()
    };

    mailer.send(email_message).await.map_err(|e| e.to_string())?;
    Ok(())
}

fn generate_jwt(user_id: i32) -> Result<String, jsonwebtoken::errors::Error> {
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET required");

    let expiration = Utc::now()
        .checked_add_signed(Duration::days(30))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

/// Middleware, который:
/// - берет `Authorization: Bearer <jwt>`
/// - валидирует JWT
/// - кладет `i32 user_id` в `req.extensions_mut()`
pub struct JwtMiddleware;

impl<S, B> Transform<S, ServiceRequest> for JwtMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = JwtMiddlewareInner<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtMiddlewareInner { service }))
    }
}

pub struct JwtMiddlewareInner<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for JwtMiddlewareInner<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let secret = match std::env::var("JWT_SECRET") {
            Ok(s) => s,
            Err(_) => {
                return Box::pin(async move {
                    Err(actix_web::error::ErrorInternalServerError(
                        "JWT secret not set",
                    ))
                });
            }
        };

        let auth_header = req
            .headers()
            .get(actix_web::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            match decode::<Claims>(
                token,
                &DecodingKey::from_secret(secret.as_ref()),
                &Validation::default(),
            ) {
                Ok(token_data) => {
                    req.extensions_mut().insert(token_data.claims.sub);
                    let fut = self.service.call(req);
                    return Box::pin(async move { fut.await });
                }
                Err(_) => {
                    return Box::pin(async move {
                        Err(actix_web::error::ErrorUnauthorized("Invalid token"))
                    });
                }
            }
        }

        Box::pin(async move {
            Err(actix_web::error::ErrorUnauthorized(
                "Missing or invalid Authorization header",
            ))
        })
    }
}
