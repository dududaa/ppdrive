use crate::{
    errors::AppError,
    state::AppState,
    utils::sqlx_utils::{SqlxFilters, SqlxValues, ToQuery},
};

#[derive(sqlx::FromRow)]
pub struct Client {
    id: String,
    key: String,
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

    pub async fn create(state: &AppState, id: &str, key: &str) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let values = SqlxValues(2).to_query(bn);
        let query = format!("INSERT INTO clients (id, key) {values}");

        sqlx::query(&query)
            .bind(&id)
            .bind(key)
            .execute(&conn)
            .await?;

        Ok(())
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}
