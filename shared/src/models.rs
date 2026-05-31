use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx_qb::prelude::*;
use uuid::Uuid;

#[derive(Serialize, Deserialize, QbModel)]
#[model(table_name = "clients")]
pub struct Client {
    id: i64,
    pid: Uuid,
    key: String,
    name: String,
    max_bucket_size: Option<f64>,
    created_at: DateTime<Utc>,
}