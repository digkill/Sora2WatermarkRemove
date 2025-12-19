// src/api/auth.rs

use actix_web::{post, web, HttpResponse, Responder, HttpMessage};
use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::task::{Context, Poll};

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: i32,
    exp: usize,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: i32,
}

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
        r#"INSERT INTO users (username, email, password_hash)
           VALUES ($1, $2, $3)
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

    let token = match generate_jwt(user_id) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("jwt encode error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(AuthResponse { token, user_id })
}

#[post("/auth/login")]
pub async fn login(state: web::Data<AppState>, payload: web::Json<LoginRequest>) -> impl Responder {
    let row = match sqlx::query(r#"SELECT id, password_hash FROM users WHERE email = $1"#)
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

    let token = match generate_jwt(user_id) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("jwt encode error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().json(AuthResponse { token, user_id })
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

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let secret = match std::env::var("JWT_SECRET") {
            Ok(s) => s,
            Err(_) => {
                return Box::pin(async move {
                    Err(actix_web::error::ErrorInternalServerError(
                        "JWT secret not set",
                    ))
                })
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
                    })
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
