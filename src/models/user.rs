use std::path::Path;

use crate::{
    errors::AppError,
    routes::CreateUserOptions,
    state::AppState,
    utils::{
        sqlx_ext::AnyDateTime,
        sqlx_utils::{SqlxFilters, SqlxValues, ToQuery},
    },
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::{asset::Asset, permission::AssetPermission, IntoSerializer};

#[derive(FromRow)]
pub struct User {
    id: i32,
    pid: String,

    #[sqlx(try_from = "i16")]
    role: UserRole,

    root_folder: Option<String>,
    folder_max_size: Option<i64>,
    email: Option<String>,
    password: Option<String>,

    #[sqlx(try_from = "String")]
    created_at: AnyDateTime,
}

impl User {
    pub async fn get(state: &AppState, user_id: &i32) -> Result<Self, AppError> {
        let conn = state.db_pool().await;

        let bn = state.backend_name();
        let filters = SqlxFilters::new("id", 1).to_query(bn);

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
        let filters = SqlxFilters::new("pid", 1).to_query(bn);

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
        let filters = SqlxFilters::new("root_folder", 1).to_query(bn);

        let query = format!("SELECT * FROM users WHERE {filters}");

        let user = sqlx::query_as::<_, User>(&query)
            .bind(root_folder)
            .fetch_one(&conn)
            .await;

        user.ok()
    }

    pub async fn create(state: &AppState, data: CreateUserOptions) -> Result<String, AppError> {
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

        let user_role: i16 = data.role.into();
        let pid = Uuid::new_v4();
        let created_at = Utc::now().naive_local();

        let bn = state.backend_name();
        let values = SqlxValues(5, 1).to_query(bn);

        let query = format!(
            "INSERT INTO users (pid, root_folder, folder_max_size, role, created_at) {values}"
        );

        sqlx::query(&query)
            .bind(pid.to_string())
            .bind(&data.root_folder)
            .bind(data.folder_max_size)
            .bind(user_role)
            .bind(created_at.to_string())
            .execute(&conn)
            .await?;

        let user = User::get_by_pid(state, &pid.to_string()).await?;
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

        let filters = SqlxFilters::new("id", 1).to_query(bn);
        let query = format!("DELETE from users WHERE {filters}");

        sqlx::query(&query).bind(self.id).execute(&conn).await?;

        Ok(())
    }

    /// Removes user permissions and assets. To be called inside or after [User::delete].
    async fn clean_up(
        state: &AppState,
        user_id: &i32,
        root_folder: &Option<String>,
    ) -> Result<(), AppError> {
        // delete root_folder
        if let Some(root_folder) = root_folder {
            let path = Path::new(root_folder);
            tokio::fs::remove_dir(path).await?;
        }

        AssetPermission::delete_for_user(state, user_id).await?;
        Asset::delete_for_user(state, user_id, root_folder.is_none()).await?;
        Ok(())
    }

    pub fn root_folder(&self) -> &Option<String> {
        &self.root_folder
    }

    pub fn id(&self) -> &i32 {
        &self.id
    }

    pub fn role(&self) -> &UserRole {
        &self.role
    }

    pub fn folder_max_size(&self) -> &Option<i64> {
        &self.folder_max_size
    }
}

#[derive(Serialize)]
pub struct UserSerializer {
    id: String,
    email: Option<String>,
    password: Option<String>,
    created_at: String,
}

impl IntoSerializer for User {
    type Serializer = UserSerializer;

    async fn into_serializer(self, _: &AppState) -> Result<Self::Serializer, AppError> {
        let User {
            pid: id,
            email,
            password,
            created_at,
            ..
        } = self;

        Ok(UserSerializer {
            id,
            email,
            password,
            created_at: created_at.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        errors::AppError,
        main_test::pretest,
        models::user::{User, UserRole},
        routes::CreateUserOptions,
    };

    #[tokio::test]
    async fn test_create_user() -> Result<(), AppError> {
        let state = pretest().await?;
        let data = CreateUserOptions {
            root_folder: Some("test_user".to_string()),
            folder_max_size: None,
            role: UserRole::Basic,
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

#[derive(Deserialize, Clone)]
pub enum UserRole {
    /// can only read assets
    Basic,

    /// full asset management
    Manager,

    /// full application management
    Admin,
}

impl TryFrom<i16> for UserRole {
    type Error = AppError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        use UserRole::*;

        if value == 0 {
            Ok(Basic)
        } else if value == 1 {
            Ok(Manager)
        } else if value == 2 {
            Ok(Admin)
        } else {
            Err(AppError::AuthorizationError(format!(
                "invalid user_role '{value}' "
            )))
        }
    }
}

impl From<UserRole> for i16 {
    fn from(value: UserRole) -> Self {
        use UserRole::*;

        match value {
            Basic => 0,
            Manager => 1,
            Admin => 2,
        }
    }
}
