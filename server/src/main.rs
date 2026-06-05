mod app;
mod middlewares;
mod payloads;
mod resp;
mod routers;
mod state;
mod utils;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::serve().await
}
