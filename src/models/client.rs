use crate::{
    errors::AppError,
    state::AppState,
    utils::sqlx_utils::{SqlxFilters, SqlxValues, ToQuery},
};

use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct Client {
    id: String,
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

    pub async fn create(state: &AppState) -> Result<String, AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let values = SqlxValues(1).to_query(bn);
        let query = format!("INSERT INTO clients (id) {values}");

        let uid = Uuid::new_v4();
        let id = uid.to_string();

        sqlx::query(&query).bind(&id).execute(&conn).await?;

        tracing::info!("client created successfully!");

        Ok(id)
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}
