use chrono::{DateTime, Utc};
use nanoid::nanoid;
use sqlx::{AnyPool, FromRow};
use sqlx_qb::prelude::*;

#[derive(QbModel, FromRow)]
#[model(table_name = "clients")]
pub struct Client {
    id: i32,
    pid: String,
    key: String,
    name: String,
    max_bucket_size: Option<f64>,
    created_at: DateTime<Utc>,
}

impl Client {
    pub async fn create(db: &DbPool, args: ClientInsertArgs) -> anyhow::Result<String> {
        let id = args.insert(db).await?;
        Ok(id)
    }

    pub async fn get_info(db: &DbPool, key: &str) -> anyhow::Result<ClientInfo> {
        let modifiers = QueryModifiers::new().with_filter(("key", key)).with_limit(1);
        let mut qb = QB::<Self>::new(db).await?;
        qb.set_modifiers(&modifiers);

        let data = qb.select_fields(["pid AS id", "name", "max_bucket_size", "key", "created_at"]).await?;
        Ok(data)
    }
    
    pub async fn all(db: &DbPool) -> anyhow::Result<Vec<ClientInfo>> {
        let mut qb = QB::<Self>::new(db).await?;
        let data = qb.select_fields_all(["pid AS id", "name", "max_bucket_size", "key", "created_at"]).await?;
        Ok(data)
    }
    
    pub async fn update_key(db: &DbPool, id: &str) -> anyhow::Result<String> {
        let modifiers = QueryModifiers::new().with_filter(("pid", id)).with_limit(1);
        let mut qb = QB::<Self>::new(db).await?;
        qb.set_modifier(modifiers);
        
        let key = Self::generate_key();
        let map = query_map!{ "key": &key };
        qb.update(map).await?;
        
        Ok(key)
    }
    
    pub fn generate_key() -> String {
        nanoid!(10, &nanoid::alphabet::SAFE)
    }
}

#[derive(FromRow)]
pub struct ClientInfo {
    pub id: String,
    pub name: String,
    key: String,
    pub created_at: String,
    pub max_bucket_size: Option<f64>
}

impl ClientInfo {
    pub fn id(&self) -> &str {
        &self.id
    }
    
    pub fn key(&self) -> &str {
        &self.key
    }
}

pub struct ClientInsertArgs<'a> {
    pub name: &'a str,
    pub key: &'a str,
    pub max_bucket_size: Option<f64>,
}

impl ModelInsertArg<Client> for ClientInsertArgs {
    type Returns = String;

    fn insert(
        self,
        db_pool: &AnyPool,
    ) -> impl Future<Output = Result<Self::Returns, sqlx::Error>> + Send {
        async {
            let pid = nanoid!();
            sqlx::query!(
            "INSERT INTO clients (pid, name, key, max_bucket_size) VALUES ($1, $2, $3, $4)",
                &pid,
                self.name,
                self.key,
                self.max_bucket_size,
            ).execute(db_pool).await?;

            Ok(pid)
        }
    }
}
