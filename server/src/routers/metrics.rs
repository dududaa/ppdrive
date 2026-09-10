//! Tower middleware for recording HTTP request metrics via the `metrics` crate.
//!
//! Emits `http_requests_total` (counter) and `http_request_duration_seconds` (histogram)
//! for every inbound request, labelled by method, route, and status.

use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use std::time::Instant;
use tower::Layer;
use tower::Service;

/// Layer that instruments every request with metrics recording.
#[derive(Clone)]
pub struct MetricsLayer;

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsService { inner }
    }
}

/// Tower service that records metrics for each request.
#[derive(Clone)]
pub struct MetricsService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for MetricsService<S>
where
    S: Service<Request<Body>, Response = Response> + Send + Clone + 'static,
    S::Future: Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        let method = req.method().to_string();
        let path = req
            .extensions()
            .get::<axum::extract::MatchedPath>()
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| req.uri().path().to_string());

        Box::pin(async move {
            let start = Instant::now();
            let res = inner.call(req).await?;
            let elapsed = start.elapsed().as_secs_f64();

            let status = res.status().as_u16().to_string();

            metrics::counter!(
                "http_requests_total",
                "method" => method.clone(),
                "route" => path.clone(),
                "status" => status.clone(),
            )
            .increment(1);

            metrics::histogram!(
                "http_request_duration_seconds",
                "method" => method,
                "route" => path,
                "status" => status,
            )
            .record(elapsed);

            Ok(res)
        })
    }
}
