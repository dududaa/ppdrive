use crate::{
    errors::AppError,
    state::AppState,
    utils::sqlx_utils::{SqlxFilters, SqlxValues, ToQuery},
};

use uuid::Uuid;

pub struct CreateClientOpts {
    pub key: Vec<u8>,
    pub payload: Vec<u8>,
}

#[derive(sqlx::FromRow)]
pub struct Client {
    id: String,
    enc_key: Vec<u8>,
    enc_payload: Vec<u8>,
}

impl Client {
    pub async fn get(state: &AppState, id: &str) -> Result<Self, AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let filters = SqlxFilters::new("id").to_query(bn);
        let query = format!("SELECT * FROM clients WHERE {filters}");

        let client = sqlx::query_as::<_, Client>(&query)
            .bind(id)
            .fetch_one(&conn)
            .await?;

        Ok(client)
    }

    pub async fn create(state: &AppState, opts: CreateClientOpts) -> Result<String, AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let values = SqlxValues(3).to_query(bn);
        let query = format!("INSERT INTO clients (id, enc_payload, enc_key) {values}");

        let uid = Uuid::new_v4();
        let id = uid.to_string();

        sqlx::query(&query)
            .bind(&id)
            .bind(&opts.payload)
            .bind(opts.key)
            .execute(&conn)
            .await?;

        tracing::info!("client created successfully!");

        Ok(id)
    }

    pub fn enc_key(&self) -> &Vec<u8> {
        &self.enc_key
    }

    pub fn enc_payload(&self) -> &Vec<u8> {
        &self.enc_payload
    }
}
