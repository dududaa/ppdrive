use crate::middlewares::ClientMiddleware;
use crate::payloads::UploadUrlConfig;
use crate::resp::{api_error, api_response, ApiResponse};
use crate::state::AppState;
use crate::utils::{create_jwt, Claims};
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

#[cfg(test)]
mod tests {
    use crate::app::create_app;
    use crate::payloads::{AssetType, UploadUrlConfig, UploadUrlMethod};
    use crate::state::AppState;
    use axum_test::TestServer;
    use shared::client::create_client;

    #[tokio::test]
    async fn test_create_signed_url() -> anyhow::Result<()> {
        let (app, _) = create_app().await?;
        let state = AppState::new().await?;
        
        let client_header_key = state.config().client_header_key.clone();
        let client = create_client(state.pool(), state.secrets(), "Test Client", None).await?;
        
        let server = TestServer::new(app);
        let config = UploadUrlConfig {
            method: UploadUrlMethod::Post,
            asset_type: AssetType::File,
            expires: 30,
            create_parents: None,
            overwrite: None,
        };

        let base_request = || server
            .post("/upload/signed")
            .json(&config)
            .content_type("application/json");
        
        
        
        let mut resp = base_request().await;
        resp.assert_status_unauthorized();

        resp = base_request().add_header(client_header_key, client.token()).await;
        resp.assert_status_ok();

        Ok(())
    }
}
