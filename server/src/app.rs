//! HTTP application construction.
//!
//! Assembles the Axum [`Router`] with CORS, tracing, upload routes,
//! static file serving, and shared [`AppState`].

use crate::routers::download::serve_direct_transformed;
use crate::routers::{
    MetricsLayer, auth_routes, bucket_routes, download_serve_routes, download_sign_routes,
    upload_routes,
};
use axum::Router;
use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::header::{
    ACCEPT, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_ORIGIN, AUTHORIZATION, CONTENT_TYPE,
};
use axum::http::{HeaderName, HeaderValue, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use metrics_exporter_prometheus::PrometheusHandle;
use ppdrive::db::bucket;
use ppdrive::plugin::PluginRegistry;
use ppdrive::plugin::loader::{LoadedPlugin, PluginDispatcher};
use ppdrive::state::AppState;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::OnceLock;
use std::task::{Context, Poll};
use std::time::Duration;
use tower::Service;
use tower_governor::GovernorLayer;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing::info_span;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// Why a registry entry was not loaded at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PluginProblem {
    /// Installed but switched off with `ppdrive plugin deactivate`.
    Deactivated,
    /// Active in the registry but the library failed to load.
    LoadFailed,
}

/// Holds the plugin libraries loaded from the registry. The handles must
/// live for the entire server lifetime — dropping one unloads its `.so`.
struct LivePlugins {
    plugins: Vec<LoadedPlugin>,
    /// Registry entries that were not loaded, keyed by short id.
    problems: HashMap<String, PluginProblem>,
}

impl LivePlugins {
    async fn get() -> &'static Self {
        if let Some(loaded) = LIVE_PLUGINS.get() {
            return loaded;
        }

        let registry = PluginRegistry::load()
            .await
            .expect("unable to load plugin registry");

        LIVE_PLUGINS.get_or_init(|| {
            let libs_dir =
                PluginRegistry::libs_dir().expect("unable to load plugin library directory");
            let mut plugins = Vec::new();
            let mut problems = HashMap::new();

            for entry in registry.list() {
                // Only activated plugins are loaded; the rest stay installed
                // and can be flipped back on with `ppdrive plugin activate`.
                if !entry.active {
                    tracing::info!(plugin = %entry.full_id(), "plugin is deactivated, skipping");
                    problems.insert(entry.id.clone(), PluginProblem::Deactivated);
                    continue;
                }

                match LoadedPlugin::load(&libs_dir.join(&entry.filename), entry.id.clone()) {
                    Ok(plugin) => plugins.push(plugin),
                    Err(err) => {
                        tracing::warn!("failed to load plugin '{}': {err}", entry.full_id());
                        problems.insert(entry.id.clone(), PluginProblem::LoadFailed);
                    }
                }
            }

            Self { plugins, problems }
        })
    }

    pub fn find(&self, id: &str) -> Option<&LoadedPlugin> {
        self.plugins.iter().find(|entry| entry.id() == id)
    }
}

static LIVE_PLUGINS: OnceLock<LivePlugins> = OnceLock::new();

/// Load a plugin by short id (e.g. `image-conversion`), or explain why it
/// is unavailable: not installed, deactivated with `ppdrive plugin
/// deactivate`, or present in the registry but failed to load.
pub(crate) async fn require_plugin(id: &str) -> Result<&'static LoadedPlugin, String> {
    let live = LivePlugins::get().await;
    if let Some(plugin) = live.find(id) {
        return Ok(plugin);
    }
    Err(match live.problems.get(id) {
        Some(PluginProblem::Deactivated) => {
            format!("{id} plugin is installed but deactivated; run `ppdrive plugin activate {id}`")
        }
        Some(PluginProblem::LoadFailed) => format!("{id} plugin failed to load"),
        None => format!("{id} plugin is not installed"),
    })
}

/// Convert whitelisted url to axum AllowOrigin. When no url is provided, all origins will be allowed.
fn whitelist_to_origins(origins: &Option<Vec<String>>) -> AllowOrigin {
    match origins {
        Some(list) => {
            let headers: Vec<HeaderValue> = list
                .iter()
                .filter_map(|s| match s.parse::<HeaderValue>() {
                    Ok(url) => Some(url),
                    Err(err) => {
                        tracing::error!("unable to pass cors origin {s}: {err}");
                        None
                    }
                })
                .collect();

            headers.into()
        }
        None => AllowOrigin::any(),
    }
}

/// Wraps a `ServeDir` mount so `?image_transformation=` requests are served
/// as transformed renditions; every other request passes through untouched.
///
/// `base` is the filesystem directory backing the mount — the transform
/// handler resolves files under it with the same traversal discipline as
/// `ServeDir`.
#[derive(Clone)]
struct TransformMount<S> {
    inner: S,
    base: PathBuf,
    state: AppState,
}

impl<S> Service<Request<Body>> for TransformMount<S>
where
    S: Service<Request<Body>, Error = std::convert::Infallible> + Clone + Send + Sync + 'static,
    S::Response: IntoResponse,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let wants_transform = request.method() == Method::GET
            && request.uri().query().is_some_and(|query| {
                query
                    .split('&')
                    .any(|pair| pair.split('=').next() == Some("image_transformation"))
            });

        if wants_transform {
            let state = self.state.clone();
            let base = self.base.clone();
            Box::pin(async move { Ok(serve_direct_transformed(state, base, request).await) })
        } else {
            let mut inner = self.inner.clone();
            Box::pin(async move {
                match inner.call(request).await {
                    Ok(response) => Ok(response.into_response()),
                    Err(infallible) => match infallible {},
                }
            })
        }
    }
}

/// Build the complete Axum application: router, CORS, tracing, static dirs, state.
///
/// Returns the [`Router`], the configured port, and live plugin handles that
/// **must** be kept alive for the server's entire lifetime.
pub async fn create_app() -> anyhow::Result<(Router, u16, DynamicRouter)> {
    create_app_inner(true).await
}

/// Build the application without the rate-limiting governor layer.
///
/// Used by integration tests where `axum_test` does not provide the
/// `ConnectInfo<SocketAddr>` extension that `SmartIpKeyExtractor` requires.
pub async fn create_test_app() -> anyhow::Result<(Router, u16, DynamicRouter)> {
    create_app_inner(false).await
}

async fn create_app_inner(
    enable_rate_limiting: bool,
) -> anyhow::Result<(Router, u16, DynamicRouter)> {
    start_logger()?;
    let state = AppState::with_broker().await?;
    let origins = state.config().allowed_origins.clone();

    let client_header_key = state.config().client_header_key.clone();
    let port = state.config().port.unwrap_or(8000);

    if origins.is_none() {
        tracing::warn!("CORS: no allowed_origins configured, all origins are permitted");
    }

    let cors = CorsLayer::new()
        .allow_origin(whitelist_to_origins(&origins))
        .allow_headers([
            ACCEPT,
            ACCESS_CONTROL_ALLOW_HEADERS,
            ACCESS_CONTROL_ALLOW_ORIGIN,
            CONTENT_TYPE,
            AUTHORIZATION,
            HeaderName::from_str(&client_header_key)?,
        ])
        .allow_methods(Any);

    // Per-IP rate limiter: 100 requests/sec with burst of 200
    let governor_conf = if enable_rate_limiting {
        let mut builder = GovernorConfigBuilder::default();
        builder.per_second(100).burst_size(200);
        let mut builder = builder.key_extractor(SmartIpKeyExtractor);
        // Emit x-ratelimit-limit / x-ratelimit-remaining on responses,
        // as documented in the upload API reference.
        let mut builder = builder.use_headers();
        Some(
            builder
                .finish()
                .ok_or_else(|| anyhow::anyhow!("failed to build rate limiter config"))?,
        )
    } else {
        None
    };

    let mut app = Router::new()
        .route("/health", get(|| async { StatusCode::OK }))
        .route("/metrics", get(metrics_handler))
        .nest("/auth", auth_routes())
        .nest("/buckets", bucket_routes())
        .nest("/upload", upload_routes())
        .nest("/download", download_sign_routes())
        .nest("/download", download_serve_routes())
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
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
        );

    let root = state.config().root_dir().unwrap_or_default();
    let paths = bucket::get_public_paths(state.db())
        .await
        .unwrap_or_else(|e| {
            tracing::error!("failed to load public bucket paths: {e}");
            vec![]
        });

    for path in &paths {
        // Strip leading '/' to get a relative filesystem path, then resolve against root.
        let relative = path.trim_start_matches('/');
        let base = root.join(relative);
        app = app.nest_service(
            path,
            TransformMount {
                inner: ServeDir::new(base.clone()),
                base,
                state: state.clone(),
            },
        );
    }

    for folder in state.config().static_folders.clone() {
        let mount_path = folder.path.unwrap_or(format!("/{}", folder.name));
        let base = root.join(&folder.name);
        app = app.nest_service(
            &mount_path,
            TransformMount {
                inner: ServeDir::new(base.clone()),
                base,
                state: state.clone(),
            },
        );
    }

    let plugins = LivePlugins::get().await;
    let mut dispatcher = PluginDispatcher::<Router<AppState>>::new();

    // Load router plugins — keep handles alive so the .so stays loaded
    if let Some(plugin) = plugins.find("dashboard") {
        let dispatched = dispatcher.dispatch(plugin, state.clone())?;
        app = app.merge(dispatched.clone());
    }

    let mut app = app
        .layer(cors)
        .layer(MetricsLayer)
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ));

    if let Some(governor_conf) = governor_conf {
        app = app.layer(GovernorLayer::new(governor_conf));
    }

    let app = app.with_state(state);
    Ok((app, port, dispatcher))
}

/// Initialize the tracing subscriber with an env-filter (defaults to `info`).
fn start_logger() -> anyhow::Result<()> {
    if let Err(err) = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer())
        .try_init()
    {
        eprintln!("logger error: {err}");
    }

    Ok(())
}

/// Prometheus metrics endpoint handler.
async fn metrics_handler() -> Result<String, StatusCode> {
    let handle = PROMETHEUS_HANDLE
        .get()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(handle.render())
}
static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the Prometheus recorder and store the handle for the `/metrics` endpoint.
pub fn install_metrics() -> anyhow::Result<()> {
    use metrics_exporter_prometheus::PrometheusBuilder;

    let builder = PrometheusBuilder::new();
    let handle = builder.install_recorder()?;
    PROMETHEUS_HANDLE
        .set(handle)
        .map_err(|_| anyhow::anyhow!("metrics already initialized"))?;
    Ok(())
}

pub type DynamicRouter = PluginDispatcher<Router<AppState>>;
