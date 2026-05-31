use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx_qb::prelude::*;

#[derive(QbModel, FromRow)]
#[model(table_name = "clients")]
pub struct Client {
    id: i64,
    pid: String,
    key: String,
    name: String,
    max_bucket_size: Option<f64>,
    created_at: DateTime<Utc>,
}