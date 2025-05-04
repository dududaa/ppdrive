use std::{fmt::Display, path::Path};

use crate::{
    errors::AppError,
    models::PermissionGroup,
    routes::admin::CreateUserRequest,
    state::AppState,
    utils::{
        sqlx_ext::AnyDateTime,
        sqlx_utils::{SqlxFilters, SqlxValues, ToQuery},
    },
};
use chrono::Utc;
use serde::Serialize;
use sqlx::{AnyPool, FromRow};
use uuid::Uuid;

use super::{IntoSerializer, Permission};

#[derive(FromRow)]
pub struct User {
    pub id: i32,
    pid: String,
    permission_group: i16,
    root_folder: Option<String>,
    folder_max_size: Option<i64>,
    #[sqlx(try_from = "String")]
    created_at: AnyDateTime,
}

impl User {
    pub async fn get(state: &AppState, user_id: &i32) -> Result<Self, AppError> {
        let conn = state.db_pool().await;

        let bn = state.backend_name();
        let filters = SqlxFilters::new("id").to_query(&bn);

        let query = format!(r#"SELECT * FROM users WHERE {filters}"#);

        let user = sqlx::query_as::<_, User>(&query)
            .bind(user_id)
            .fetch_one(&conn)
            .await?;

        Ok(user)
    }

    pub async fn get_by_pid(state: &AppState, pid: &str) -> Result<Self, AppError> {
        let conn = state.db_pool().await;

        let bn = state.backend_name();
        let filters = SqlxFilters::new("pid").to_query(&bn);

        let query = format!(r#"SELECT * FROM users WHERE {filters}"#);

        let user = sqlx::query_as::<_, User>(&query)
            .bind(pid)
            .fetch_one(&conn)
            .await?;

        Ok(user)
    }

    pub async fn get_by_root_folder(state: &AppState, root_folder: &str) -> Option<Self> {
        let conn = state.db_pool().await;

        let bn = state.backend_name();
        let filters = SqlxFilters::new("root_folder").to_query(&bn);

        let query = format!("SELECT * FROM users WHERE {filters}");

        let user = sqlx::query_as::<_, User>(&query)
            .bind(root_folder)
            .fetch_one(&conn)
            .await;

        user.ok()
    }

    pub async fn create(state: &AppState, data: CreateUserRequest) -> Result<String, AppError> {
        let conn = state.db_pool().await;

        // check if someone already owns root folder
        if let Some(folder) = &data.root_folder {
            if User::get_by_root_folder(state, folder).await.is_some() {
                return Err(AppError::InternalServerError(
                        format!("user with root_folder: '{folder}' already exists. please provide unique folder name")
                    ));
            }

            let path = Path::new(folder);
            tokio::fs::create_dir_all(path).await?;
        }

        let pg: i16 = data.permission_group.clone().into();
        let pid = Uuid::new_v4();
        let created_at = Utc::now().naive_local();

        let bn = state.backend_name();
        let values = SqlxValues(5).to_query(bn);

        let query = format!(
            r#"
            INSERT INTO users (permission_group, pid, root_folder, folder_max_size, created_at)
            {values}
        "#
        );

        sqlx::query(&query)
            .bind(pg)
            .bind(pid.to_string())
            .bind(&data.root_folder)
            .bind(&data.folder_max_size)
            .bind(created_at.to_string())
            .execute(&conn)
            .await?;

        let user = User::get_by_pid(&state, &pid.to_string()).await?;

        if let PermissionGroup::Custom = data.permission_group {
            if let Some(perms) = data.permissions {
                for perm in perms {
                    UserPermission::create(&conn, &user.id, perm).await?;
                }
            }
        }

        Ok(user.pid)
    }

    pub async fn delete(state: &AppState, user_id: &i32) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let filters = SqlxFilters::new("id").to_query(bn);
        let query = format!("DELETE from users WHERE {filters}");

        sqlx::query(&query).bind(user_id).execute(&conn).await?;

        Ok(())
    }

    pub async fn permissions(&self, state: &AppState) -> Result<Option<Vec<Permission>>, AppError> {
        let pg = PermissionGroup::try_from(self.permission_group)?;
        let perms = match pg {
            PermissionGroup::Custom => {
                let conn = state.db_pool().await;
                let bn = state.backend_name();

                let filters = SqlxFilters::new("id").to_query(bn);
                let query = format!("SELECT * FROM user_permissions WHERE {filters}");

                let user_perms = sqlx::query_as::<_, UserPermission>(&query)
                    .bind(self.id)
                    .fetch_all(&conn)
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

    pub fn root_folder(&self) -> &Option<String> {
        &self.root_folder
    }

    pub fn permission_group(&self) -> &i16 {
        &self.permission_group
    }
}

#[derive(FromRow)]
pub struct UserPermission {
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

    async fn into_serializer(self, state: &AppState) -> Result<Self::Serializer, AppError> {
        let permissions = self.permissions(state).await?;

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

#[cfg(test)]
mod tests {
    use crate::{
        errors::AppError,
        models::{user::User, PermissionGroup},
        routes::admin::CreateUserRequest,
        state::AppState,
    };

    #[tokio::test]
    async fn test_create_user() -> Result<(), AppError> {
        dotenv::dotenv().ok();
        match AppState::new().await {
            Ok(state) => {
                let data = CreateUserRequest {
                    permissions: None,
                    permission_group: PermissionGroup::Full,
                    root_folder: Some("test_user".to_string()),
                    folder_max_size: None,
                };

                let user = User::create(&state, data).await;

                assert!(user.is_ok());
            }
            Err(err) => println!("unable to create state: {err}"),
        }

        Ok(())
    }
}
