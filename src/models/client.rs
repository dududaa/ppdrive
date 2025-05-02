use crate::errors::AppError;
use sqlx::AnyPool;

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
        let client = sqlx::query_as::<_, Client>(
            r#"
                INSERT INTO clients (enc_payload, enc_key)
                VALUES(?, ?)
            "#,
        )
        .bind(&opts.payload)
        .bind(opts.key)
        .fetch_one(conn)
        .await?;

        Ok(client)
    }
}
