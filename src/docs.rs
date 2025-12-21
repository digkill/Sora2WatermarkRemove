use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::api::auth::register,
        crate::api::auth::login,
        crate::api::handlers::upload,
        crate::api::webhooks::watermark_callback,
        crate::api::webhooks::watermark_callback_alias
    ),
    components(
        schemas(
            crate::api::auth::RegisterRequest,
            crate::api::auth::LoginRequest,
            crate::api::auth::AuthResponse,
            crate::api::handlers::UrlUploadBody,
            crate::api::handlers::UploadResponse,
            crate::api::webhooks::CallbackPayload,
            crate::api::webhooks::CallbackData,
            crate::api::webhooks_lava::LavaWebhook
        )
    ),
    tags(
        (name = "auth", description = "Authentication"),
        (name = "uploads", description = "Video uploads"),
        (name = "webhooks", description = "Callbacks from Kie.ai")
    )
)]
pub struct ApiDoc;
