use std::path::Path;

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
use sqlx::FromRow;
use uuid::Uuid;

use super::{asset::Asset, IntoSerializer, Permission};

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
        let filters = SqlxFilters::new("id").to_query(bn);

        let query = format!("SELECT * FROM users WHERE {filters}");

        let user = sqlx::query_as::<_, User>(&query)
            .bind(user_id)
            .fetch_one(&conn)
            .await?;

        Ok(user)
    }

    pub async fn get_by_pid(state: &AppState, pid: &str) -> Result<Self, AppError> {
        let conn = state.db_pool().await;

        let bn = state.backend_name();
        let filters = SqlxFilters::new("pid").to_query(bn);

        let query = format!("SELECT * FROM users WHERE {filters}");

        let user = sqlx::query_as::<_, User>(&query)
            .bind(pid)
            .fetch_one(&conn)
            .await?;

        Ok(user)
    }

    pub async fn get_by_root_folder(state: &AppState, root_folder: &str) -> Option<Self> {
        let conn = state.db_pool().await;

        let bn = state.backend_name();
        let filters = SqlxFilters::new("root_folder").to_query(bn);

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

        let query = format!("INSERT INTO users (permission_group, pid, root_folder, folder_max_size, created_at) {values}");

        sqlx::query(&query)
            .bind(pg)
            .bind(pid.to_string())
            .bind(&data.root_folder)
            .bind(data.folder_max_size)
            .bind(created_at.to_string())
            .execute(&conn)
            .await?;

        let user = User::get_by_pid(state, &pid.to_string()).await?;

        if let PermissionGroup::Custom = data.permission_group {
            if let Some(perms) = data.permissions {
                for perm in perms {
                    UserPermission::create(state, &user.id, perm).await?;
                }
            }
        }

        Ok(user.pid)
    }

    pub async fn delete(&self, state: &AppState) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let ss = state.clone();
        let user_id = self.id;
        let root_folder = self.root_folder().clone();

        tokio::task::spawn(async move {
            if let Err(err) = User::clean_up(&ss, &user_id, &root_folder).await {
                tracing::error!("user clean up failed: {err}")
            }
        });

        let filters = SqlxFilters::new("id").to_query(bn);
        let query = format!("DELETE from users WHERE {filters}");

        sqlx::query(&query).bind(self.id).execute(&conn).await?;

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

    async fn delete_permissions(state: &AppState, user_id: &i32) -> Result<(), AppError> {
        UserPermission::delete_for_user(state, user_id).await
    }

    /// Removes user permissions and assets. To be called inside or after [User::delete].
    async fn clean_up(
        state: &AppState,
        user_id: &i32,
        root_folder: &Option<String>,
    ) -> Result<(), AppError> {
        User::delete_permissions(state, user_id).await?;

        // delete root_folder
        if let Some(root_folder) = root_folder {
            let path = Path::new(root_folder);
            tokio::fs::remove_dir(path).await?;
        }

        Asset::delete_for_user(state, user_id, root_folder.is_none()).await?;
        Ok(())
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
    async fn create(state: &AppState, user_id: &i32, perm: Permission) -> Result<(), AppError> {
        let conn = state.db_pool().await;

        let val: i16 = perm.into();

        let bn = state.backend_name();
        let values = SqlxValues(2).to_query(bn);
        let query = format!("INSERT INTO user_permissions (user_id, permission) {values}");

        sqlx::query(&query)
            .bind(user_id)
            .bind(val)
            .execute(&conn)
            .await?;

        Ok(())
    }

    async fn delete_for_user(state: &AppState, user_id: &i32) -> Result<(), AppError> {
        let conn = state.db_pool().await;
        let bn = state.backend_name();

        let filters = SqlxFilters::new("user_id").to_query(bn);
        let query = format!("DELETE FROM user_permissions WHERE {filters}");

        sqlx::query(&query).bind(user_id).execute(&conn).await?;

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
        main_test::pretest,
        models::{user::User, PermissionGroup},
        routes::admin::CreateUserRequest,
    };

    #[tokio::test]
    async fn test_create_user() -> Result<(), AppError> {
        let state = pretest().await?;
        let data = CreateUserRequest {
            permissions: None,
            permission_group: PermissionGroup::Full,
            root_folder: Some("test_user".to_string()),
            folder_max_size: None,
        };

        let user_uid = User::create(&state, data).await;

        assert!(user_uid.is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_user() -> Result<(), AppError> {
        let state = pretest().await?;
        let user = User::get(&state, &4).await?;
        let delete = user.delete(&state).await;

        if let Err(err) = &delete {
            println!("user delete failed: {err}");
        }

        assert!(delete.is_ok());
        Ok(())
    }
}
