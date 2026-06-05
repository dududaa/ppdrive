use crate::middlewares::ClientMiddleware;
use crate::payloads::UploadUrlConfig;
use crate::resp::{ApiResponse, api_error, api_response};
use crate::state::AppState;
use crate::utils::{Claims, create_jwt};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use axum_macros::debug_handler;
use validator::Validate;

#[debug_handler]
async fn create_signed_url(
    State(state): State<AppState>,
    client: ClientMiddleware,
    Json(data): Json<UploadUrlConfig>,
) -> ApiResponse<String> {
    data.validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    let claims = Claims {
        sub: client.id(),
        exp: data.expires,
        data,
    };
    let token = create_jwt(state.secrets(), &claims)?;

    api_response(token)
}

pub fn upload_routes() -> Router<AppState> {
    Router::new().route("/signed", post(create_signed_url))
}
