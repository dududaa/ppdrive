use std::path::Path;

use diesel::{
    prelude::{Associations, Insertable, Queryable, Selectable},
    ExpressionMethods, QueryDsl, SelectableHelper,
};
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use tokio::fs::{create_dir_all, File};

use crate::{errors::AppError, models::user::User, state::DbPooled};

use super::AssetType;

#[derive(Queryable, Selectable, Insertable, Associations)]
#[diesel(belongs_to(User))]
#[diesel(table_name = crate::schema::assets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Asset {
    pub id: i32,
    pub asset_path: String,
    pub user_id: i32,
    pub public: bool,
}

#[derive(Default, Deserialize)]
pub struct CreateAssetOptions {
    /// Destination path where asset should be created
    pub path: String,

    /// The type of asset - whether it's a file or folder
    pub asset_type: AssetType,

    /// Asset's visibility. Public assets can be read/accessed by everyone. Private assets can be
    /// viewed ONLY by permission.
    pub public: Option<bool>,

    /// If `asset_type` is [AssetType::Folder], we determine whether we should force-create it's parents folder if they
    /// don't exist. Asset creation will result in error if `create_parents` is `false` and folder parents don't exist.
    pub create_parents: Option<bool>,

    /// If asset is file and user/owner uploaded data to write to it. The data is written in a path (`tmp_file`)
    /// by the route handler.
    pub tmp_file: Option<String>,
}

impl Asset {
    pub async fn get_by_path(conn: &mut DbPooled<'_>, path: String) -> Result<Self, AppError> {
        use crate::schema::assets::dsl::*;

        let asset = assets
            .filter(asset_path.eq(path))
            .select(Asset::as_select())
            .first(conn)
            .await
            .map_err(|err| AppError::InternalServerError(err.to_string()))?;

        Ok(asset)
    }

    pub async fn create(
        conn: &mut DbPooled<'_>,
        user: &i32,
        opts: CreateAssetOptions,
    ) -> Result<String, AppError> {
        use crate::schema::assets::dsl::*;

        let CreateAssetOptions {
            path,
            public: is_public,
            tmp_file: temp_file,
            asset_type,
            create_parents,
        } = opts;

        let user = User::get(conn, *user).await?;
        let path = user
            .root_folder
            .map_or(path.clone(), |rf| format!("{rf}/{path}"));

        let ap = Path::new(&path);

        match asset_type {
            AssetType::File => {
                if let Some(parent) = ap.parent() {
                    if !parent.exists() {
                        create_dir_all(parent).await?;
                    }
                }

                if let Err(err) = File::create(ap).await {
                    tracing::info!("unble to create file: {err}");
                    return Err(AppError::IOError(err.to_string()));
                }

                if let Some(tmp) = temp_file {
                    tokio::fs::rename(&tmp, ap).await?;
                    tokio::fs::remove_file(ap).await?;
                }
            }
            AssetType::Folder => {
                if create_parents.unwrap_or_default() {
                    tokio::fs::create_dir_all(ap).await?
                } else {
                    tokio::fs::create_dir(ap).await?;
                }
            }
        }

        diesel::insert_into(assets)
            .values((
                asset_path.eq(&path),
                public.eq(is_public.unwrap_or_default()),
                user_id.eq(user.id),
            ))
            .execute(conn)
            .await
            .map_err(|err| AppError::DatabaseError(err.to_string()))?;

        Ok(path)
    }
}
