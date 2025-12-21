use actix::{Actor, ActorContext, AsyncContext, Handler, Message, Recipient};
use actix_web::{Error, HttpRequest, HttpResponse, web};
use actix_web_actors::ws;
use chrono::{DateTime, Utc};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use serde_urlencoded;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::AppState;

static NEXT_SESSION_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Message)]
#[rtype(result = "()")]
struct WsMessage(pub String);

#[derive(Message)]
#[rtype(result = "()")]
struct Connect {
    user_id: i32,
    session_id: usize,
    addr: Recipient<WsMessage>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct Disconnect {
    user_id: i32,
    session_id: usize,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct NotifyUpload {
    pub user_id: i32,
    pub event: UploadEvent,
}

#[derive(Clone, Debug, Serialize)]
pub struct UploadEvent {
    pub event: &'static str,
    pub data: UploadEventData,
}

#[derive(Clone, Debug, Serialize)]
pub struct UploadEventData {
    pub id: i32,
    pub task_id: Option<String>,
    pub status: String,
    pub cleaned_url: Option<String>,
    pub original_filename: String,
    pub created_at: Option<DateTime<Utc>>,
}

pub struct WsHub {
    sessions: HashMap<i32, HashMap<usize, Recipient<WsMessage>>>,
}

impl WsHub {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

impl Actor for WsHub {
    type Context = actix::Context<Self>;
}

impl Handler<Connect> for WsHub {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Self::Context) -> Self::Result {
        self.sessions
            .entry(msg.user_id)
            .or_default()
            .insert(msg.session_id, msg.addr);
    }
}

impl Handler<Disconnect> for WsHub {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Self::Context) -> Self::Result {
        if let Some(user_sessions) = self.sessions.get_mut(&msg.user_id) {
            user_sessions.remove(&msg.session_id);
            if user_sessions.is_empty() {
                self.sessions.remove(&msg.user_id);
            }
        }
    }
}

impl Handler<NotifyUpload> for WsHub {
    type Result = ();

    fn handle(&mut self, msg: NotifyUpload, _: &mut Self::Context) -> Self::Result {
        if let Some(user_sessions) = self.sessions.get(&msg.user_id) {
            if let Ok(payload) = serde_json::to_string(&msg.event) {
                for addr in user_sessions.values() {
                    let _ = addr.do_send(WsMessage(payload.clone()));
                }
            }
        }
    }
}

struct WsSession {
    user_id: i32,
    session_id: usize,
    hub: actix::Addr<WsHub>,
}

impl WsSession {
    fn new(user_id: i32, hub: actix::Addr<WsHub>) -> Self {
        Self {
            user_id,
            session_id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
            hub,
        }
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hub.do_send(Connect {
            user_id: self.user_id,
            session_id: self.session_id,
            addr: ctx.address().recipient(),
        });
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        self.hub.do_send(Disconnect {
            user_id: self.user_id,
            session_id: self.session_id,
        });
    }
}

impl Handler<WsMessage> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.0);
    }
}

impl actix::StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, item: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match item {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Pong(_)) => {}
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            Ok(ws::Message::Text(_)) => {}
            Ok(ws::Message::Binary(_)) => {}
            Ok(ws::Message::Continuation(_)) => {}
            Ok(ws::Message::Nop) => {}
            Err(_) => ctx.stop(),
        }
    }
}

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

#[derive(Deserialize)]
struct Claims {
    sub: i32,
    exp: usize,
}

pub async fn uploads_ws(
    req: HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    let token = serde_urlencoded::from_str::<WsQuery>(req.query_string())
        .ok()
        .map(|q| q.token)
        .filter(|t| !t.is_empty());

    let Some(token) = token else {
        return Err(actix_web::error::ErrorUnauthorized("Missing token"));
    };

    let user_id = decode_user_id(&token)?;
    ws::start(WsSession::new(user_id, state.ws_hub.clone()), &req, stream)
}

fn decode_user_id(token: &str) -> Result<i32, Error> {
    let secret = std::env::var("JWT_SECRET")
        .map_err(|_| actix_web::error::ErrorInternalServerError("JWT secret not set"))?;
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )
    .map(|data| data.claims.sub)
    .map_err(|_| actix_web::error::ErrorUnauthorized("Invalid token"))
}

pub async fn notify_upload_by_task(
    pool: &sqlx::PgPool,
    hub: &actix::Addr<WsHub>,
    task_id: &str,
) {
    let row = sqlx::query(
        r#"SELECT id, user_id, status, cleaned_url, original_filename, created_at, task_id
           FROM uploads
           WHERE task_id = $1"#,
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await;

    let Ok(Some(row)) = row else {
        return;
    };

    let user_id: i32 = row.get("user_id");
    let event = UploadEvent {
        event: "upload.updated",
        data: UploadEventData {
            id: row.get("id"),
            task_id: row.get("task_id"),
            status: row.get("status"),
            cleaned_url: row.get("cleaned_url"),
            original_filename: row.get("original_filename"),
            created_at: row.get("created_at"),
        },
    };

    hub.do_send(NotifyUpload { user_id, event });
}
