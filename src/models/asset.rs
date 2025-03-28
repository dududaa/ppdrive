use std::path::Path;

use diesel::{prelude::{Associations, Insertable, Queryable, Selectable}, ExpressionMethods};
use diesel_async::RunQueryDsl;

use crate::{errors::AppError, models::user::User, state::DbPooled};

#[derive(Queryable, Selectable, Insertable, Associations)]
#[diesel(belongs_to(User))]
#[diesel(table_name = crate::schema::assets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Asset {
    pub id: i32,
    pub asset_path: String,
    pub user_id: i32,
    pub public: bool
}

#[derive(Default)]
pub struct CreateAssetOptions {
    pub user: i32,
    pub path: String,
    pub public: Option<bool>,
    pub tmp_file: Option<String>
}

impl Asset {
    pub async fn create(conn: &mut DbPooled<'_>, opts: CreateAssetOptions) -> Result<String, AppError> {
        use crate::schema::assets::dsl::*;
        
        let CreateAssetOptions { path, public: is_public, tmp_file: temp_file, user } = opts;
        let fp = Path::new(&path);
        
        if let Some(tmp) = temp_file {
            tokio::fs::rename(&tmp, fp).await?;
        }

        diesel::insert_into(assets)
            .values((
                asset_path.eq(&path),
                public.eq(is_public.unwrap_or(false)),
                user_id.eq(user)
            ))
            .execute(conn)
            .await
            .map_err(|err| AppError::DatabaseError(err.to_string()))?;

        Ok(path)
    }
}