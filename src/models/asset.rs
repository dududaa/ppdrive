use std::path::{Path, PathBuf};

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
}

impl Asset {
    pub async fn get_by_path(conn: &mut DbPooled<'_>, path: &str) -> Result<Self, AppError> {
        use crate::schema::assets::dsl::*;

        let asset = assets
            .filter(asset_path.eq(path))
            .select(Asset::as_select())
            .first(conn)
            .await
            .map_err(|err| AppError::InternalServerError(err.to_string()))?;

        Ok(asset)
    }

    pub async fn create_or_update(
        conn: &mut DbPooled<'_>,
        user: &i32,
        opts: CreateAssetOptions,
        temp_file: Option<PathBuf>,
    ) -> Result<String, AppError> {
        use crate::schema::assets::dsl::*;

        let CreateAssetOptions {
            path,
            public: is_public,
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
                    tokio::fs::copy(&tmp, ap).await?;
                    tokio::fs::remove_file(&tmp).await?;
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

        // try to create asset record if it doesn't exist. If exists, update
        match Self::get_by_path(conn, &path).await {
            Ok(exists) => {
                if exists.user_id == user.id {
                    diesel::update(assets.find(exists.id))
                        .set(public.eq(is_public.unwrap_or_default()))
                        .execute(conn)
                        .await
                        .map_err(|err| AppError::DatabaseError(err.to_string()))?;
                } else {
                    tokio::fs::remove_file(&path).await?;
                    return Err(AppError::AuthorizationError(
                        "user has no permission to update asset".to_string(),
                    ));
                }
            }
            Err(_) => {
                diesel::insert_into(assets)
                    .values((
                        asset_path.eq(&path),
                        public.eq(is_public.unwrap_or_default()),
                        user_id.eq(user.id),
                    ))
                    .execute(conn)
                    .await
                    .map_err(|err| AppError::DatabaseError(err.to_string()))?;
            }
        }

        Ok(path)
    }
}
