use std::env::set_var;

use axum::http::header::{
    ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::{HeaderName, HeaderValue};
use axum::{
    extract::MatchedPath,
    http::Request,
    routing::{get, IntoMakeService},
    Router,
};
use ppd_shared::config::{AppConfig, CorsOriginType};
use ppd_shared::plugins::router::ServiceRouter;
use ppd_shared::plugins::service::{ServiceAuthMode, ServiceType};
use tower_http::cors::{AllowOrigin, Any};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info_span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::errors::ServerError;
use crate::ServerResult;
use handlers::{
    get_asset,
    jwt::{BEARER_KEY, BEARER_VALUE},
    state::AppState,
};
use ppd_shared::tools::init_secrets;

fn to_origins(origins: CorsOriginType) -> AllowOrigin {
    match origins {
        CorsOriginType::Any => AllowOrigin::any(),
        CorsOriginType::List(list) => {
            let headers: Vec<HeaderValue> = list
                .iter()
                .map(|s| match s.parse::<HeaderValue>() {
                    Ok(url) => Some(url),
                    Err(err) => {
                        tracing::error!("unable to pass cors origin {s}: {err}");
                        None
                    }
                })
                .flatten()
                .collect();

            headers.into()
        }
    }
}

async fn create_app(config: &AppConfig) -> Result<IntoMakeService<Router<()>>, ServerError> {
    let state = AppState::new(config).await?;
    let origins = config.server().origins();

    let cors = CorsLayer::new()
        .allow_origin(to_origins(origins))
        .allow_headers([
            ACCEPT,
            ACCESS_CONTROL_ALLOW_HEADERS,
            ACCESS_CONTROL_ALLOW_ORIGIN,
            CONTENT_TYPE,
            AUTHORIZATION,
            HeaderName::from_static("x-ppd-client"),
        ])
        .allow_methods(Any);

    set_var(BEARER_KEY, BEARER_VALUE);

    let client_router = get_client_router(config)?;
    let router = Router::new()
        .route("/:asset_type/*asset_path", get(get_asset))
        .nest("/client", *client_router)
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                // Log the matched route's path (with placeholders not filled in).
                // Use request.uri() or OriginalUri if you want the real path.
                let matched_path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(MatchedPath::as_str);

                info_span!(
                    "http_request",
                    method = ?request.method(),
                    matched_path,
                    some_other_field = tracing::field::Empty,
                )
            }),
        )
        .layer(cors)
        .with_state(state)
        .into_make_service();

    Ok(router)
}

pub async fn initialize_app(
    config: &AppConfig,
) -> ServerResult<IntoMakeService<Router<()>>> {
    if let Err(err) = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ppdrive=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()
    {
        tracing::warn!("{err}")
    }

    // start ppdrive app
    init_secrets().await?;
    create_app(&config).await
}

fn get_client_router(config: &AppConfig) -> ServerResult<Box<Router<AppState>>> {
    let router = if config.server().auth_modes().contains(&ServiceAuthMode::Client) {
        let svc_router = ServiceRouter { svc_type: ServiceType::Rest, auth_mode: ServiceAuthMode::Client };
        svc_router.get(config)?
    } else {
        Box::new(Router::new())
    };

    Ok(router)
    
}