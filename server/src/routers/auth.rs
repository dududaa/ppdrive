use crate::routers::resp::{api_error, api_response, ApiResponse};
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use shared::db::user;
use shared::server::UserInfo;
use shared::seconds_from_now;
use validator::Validate;

#[derive(Deserialize, Validate)]
pub(crate) struct LoginRequest {
    /// User email address.
    #[validate(email)]
    pub email: String,
    /// User password.
    #[validate(length(min = 1, max = 128))]
    pub password: String,
}

#[derive(Serialize)]
pub(crate) struct LoginResponse {
    pub token: String,
    pub expires_in: i64,
}

/// Authenticate a user with email and password, returning a signed token.
///
/// `POST /auth/login`
#[axum::debug_handler]
pub(crate) async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> ApiResponse<LoginResponse> {
    req.validate()
        .map_err(|err| api_error(err).with_status_code(StatusCode::BAD_REQUEST))?;

    let user_id = user::verify_password(&req.email, &req.password, state.db())
        .await
        .map_err(|_| api_error("invalid credentials")
            .with_status_code(StatusCode::UNAUTHORIZED))?;

    let expires_in = 3600; // 1 hour
    let exp = seconds_from_now(expires_in)?;

    let info = UserInfo {
        user_email: req.email,
        exp,
    };

    // Sign with hex-encoded app secret key
    let signing_key = hex::encode(state.secrets().secret_key());
    let token = info.sign(&signing_key, state.hasher())?;

    tracing::info!(user_id = user_id, "user logged in");

    api_response(LoginResponse {
        token,
        expires_in,
    })
}
