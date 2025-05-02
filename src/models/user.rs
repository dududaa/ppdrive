use std::path::Path;

use crate::{errors::AppError, models::PermissionGroup, routes::admin::CreateUserRequest};
use serde::Serialize;
use sqlx::{AnyPool, FromRow};

use super::{IntoSerializer, Permission};

#[derive(FromRow)]
pub struct User {
    pub id: i32,
    pub pid: String,
    pub permission_group: i16,
    pub root_folder: Option<String>,
    pub folder_max_size: Option<i64>,
    pub created_at: String,
}

impl User {
    pub async fn get(conn: &AnyPool, user_id: &i32) -> Result<Self, AppError> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(conn)
            .await?;

        Ok(user)
    }

    pub async fn get_by_pid(conn: &AnyPool, pid: &str) -> Result<Self, AppError> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE pid = ?")
            .bind(pid)
            .fetch_one(conn)
            .await?;

        Ok(user)
    }

    pub async fn get_by_root_folder(conn: &AnyPool, root_folder: &str) -> Option<Self> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE root_folder = ?")
            .bind(root_folder)
            .fetch_one(conn)
            .await;

        user.ok()
    }

    pub async fn create(conn: &AnyPool, data: CreateUserRequest) -> Result<String, AppError> {
        if let Some(folder) = &data.root_folder {
            if User::get_by_root_folder(conn, folder).await.is_some() {
                return Err(AppError::InternalServerError(
                        format!("user with root_folder: '{folder}' already exists. please provide unique folder name")
                    ));
            }

            let path = Path::new(folder);
            tokio::fs::create_dir_all(path).await?;
        }

        let pg: i16 = data.permission_group.clone().into();

        let user = sqlx::query_as::<_, User>(
            r#"
                INSERT INTO users (permission_group, root_folder, folder_max_size)
                VALUES(?, ?, ?)
            "#,
        )
        .bind(pg)
        .bind(&data.root_folder)
        .bind(&data.folder_max_size)
        .fetch_one(conn)
        .await?;

        if let PermissionGroup::Custom = data.permission_group {
            if let Some(perms) = data.permissions {
                for perm in perms {
                    UserPermission::create(conn, &user.id, perm).await?;
                }
            }
        }

        Ok(user.pid)
    }

    pub async fn delete(conn: &AnyPool, user_id: &i32) -> Result<(), AppError> {
        sqlx::query("DELETE from users WHERE id = ?")
            .bind(user_id)
            .execute(conn)
            .await?;

        Ok(())
    }

    pub async fn permissions(&self, conn: &AnyPool) -> Result<Option<Vec<Permission>>, AppError> {
        let pg = PermissionGroup::try_from(self.permission_group)?;
        let perms = match pg {
            PermissionGroup::Custom => {
                let user_perms = sqlx::query_as::<_, UserPermission>(
                    "SELECT * FROM user_permissions WHERE id = ?",
                )
                .bind(self.id)
                .fetch_all(conn)
                .await?;

                let mut perms = Vec::with_capacity(user_perms.len());
                for perm in user_perms {
                    perms.push(perm.try_into()?);
                }

                Some(perms)
            }
            _ => pg.default_permissions(),
        };

        Ok(perms)
    }
}

#[derive(FromRow)]
pub struct UserPermission {
    pub id: i32,
    pub user_id: i32,
    pub permission: i16,
}

impl UserPermission {
    async fn create(conn: &AnyPool, uid: &i32, perm: Permission) -> Result<(), AppError> {
        let val: i16 = perm.into();

        sqlx::query(
            r#"
                INSERT INTO user_permissions (user_id, permission)
                VALUES(?, ?)
            "#,
        )
        .bind(uid)
        .bind(val)
        .execute(conn)
        .await?;

        Ok(())
    }
}

impl TryFrom<UserPermission> for Permission {
    type Error = AppError;

    fn try_from(value: UserPermission) -> Result<Self, Self::Error> {
        let perm = Permission::try_from(value.permission)?;
        Ok(perm)
    }
}

#[derive(Serialize)]
pub struct UserSerializer {
    pub permission_group: PermissionGroup,
    pub permissions: Option<Vec<Permission>>,
    pub created_at: String,
}

impl IntoSerializer for User {
    type Serializer = UserSerializer;

    async fn into_serializer(self, conn: &AnyPool) -> Result<Self::Serializer, AppError> {
        let permissions = self.permissions(conn).await?;

        let User {
            permission_group,
            created_at,
            ..
        } = self;

        let permission_group = PermissionGroup::try_from(permission_group)?;

        Ok(UserSerializer {
            permission_group,
            permissions,
            created_at: created_at.to_string(),
        })
    }
}
