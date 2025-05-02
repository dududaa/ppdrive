use crate::errors::AppError;
use sqlx::AnyPool;
use uuid::Uuid;

pub struct CreateClientOpts {
    pub key: Vec<u8>,
    pub payload: Vec<u8>,
}

#[derive(sqlx::FromRow)]
pub struct Client {
    pub id: i32,
    pub enc_key: Vec<u8>,
    pub enc_payload: Vec<u8>,
    pub cid: String,
}

impl Client {
    pub async fn get(conn: &AnyPool, uid: &str) -> Result<Self, AppError> {
        let client = sqlx::query_as::<_, Client>("SELECT * FROM clients WHERE cid = ?")
            .bind(uid)
            .fetch_one(conn)
            .await?;

        Ok(client)
    }

    pub async fn create(conn: &AnyPool, opts: CreateClientOpts) -> Result<Self, AppError> {
        let id = Uuid::new_v4();

        let client = sqlx::query_as::<_, Client>(
            r#"
                INSERT INTO clients (enc_payload, enc_key, cid)
                VALUES($1, $2, $3)
                RETURNING *
            "#,
        )
        .bind(&opts.payload)
        .bind(opts.key)
        .bind(id.to_string())
        .fetch_one(conn)
        .await?;

        tracing::info!("client created successfully!");

        Ok(client)
    }
}
