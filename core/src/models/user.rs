use rbatis::{RBatis, crud, impl_select, rbdc::DateTime};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

use crate::{CoreResult, errors::CoreError, options::CreateUserOptions};

use super::{IntoSerializer, asset::Assets, check_model, permission::AssetPermissions};

#[derive(Serialize, Deserialize)]
pub struct Users {
    id: u64,
    pid: String,
    role: u8,
    partition: Option<String>,
    partition_size: Option<u64>,
    email: Option<String>,
    password: Option<String>,
    created_at: DateTime,
}

crud!(Users {});
impl_select!(Users { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });

impl Users {
    pub async fn get<'a>(rb: &RBatis, user_id: &u64) -> CoreResult<Users> {
        let user = Users::get_by_key(rb, "id", user_id).await?;
        check_model(user, "user not found")
    }

    pub async fn get_by_pid(rb: &RBatis, pid: &str) -> CoreResult<Users> {
        let user = Users::get_by_key(rb, "pid", pid).await?;
        check_model(user, "user not found")
    }

    pub async fn get_by_partition_name(rb: &RBatis, partition_name: &str) -> CoreResult<Users> {
        let user = Users::get_by_key(rb, "partition", partition_name).await?;
        check_model(user, "user not found")
    }

    pub async fn create(rb: &RBatis, data: CreateUserOptions) -> CoreResult<String> {
        // check if someone already owns root folder
        if let Some(partition) = &data.partition {
            let exists = Users::get_by_partition_name(rb, partition).await;
            if exists.is_ok() {
                return Err(CoreError::ServerError(format!(
                    "user with partition_name: '{partition}' already exists. please provide unique partition name"
                )));
            }

            tokio::fs::create_dir_all(partition).await?;
        }

        let role: u8 = data.role.into();
        let pid = Uuid::new_v4().to_string();
        let created_at = DateTime::now();

        let user = Users {
            id: 0,
            pid,
            role,
            partition: data.partition,
            partition_size: data.partition_size,
            email: None,
            password: None,
            created_at,
        };

        Users::insert(rb, &user).await?;
        Ok(user.pid)
    }

    pub async fn delete(&self, rb: &RBatis) -> CoreResult<()> {
        let ss = rb.clone();
        let user_id = self.id;
        let root_folder = self.partition().clone();

        tokio::task::spawn(async move {
            if let Err(err) = Users::clean_up(&ss, &user_id, &root_folder).await {
                tracing::error!("user clean up failed: {err}")
            }
        });

        Users::delete_by_column(rb, "id", &self.id).await?;

        Ok(())
    }

    /// Removes user permissions and assets. To be called inside or after [User::delete].
    async fn clean_up(rb: &RBatis, user_id: &u64, partition: &Option<String>) -> CoreResult<()> {
        // delete root_folder
        if let Some(root_folder) = partition {
            let path = Path::new(root_folder);
            tokio::fs::remove_dir_all(path).await?;
        }

        AssetPermissions::delete_for_user(rb, user_id).await?;
        Assets::delete_for_user(rb, user_id, partition.is_none()).await?;
        Ok(())
    }

    pub fn partition(&self) -> &Option<String> {
        &self.partition
    }

    pub fn id(&self) -> &u64 {
        &self.id
    }

    pub fn role(&self) -> CoreResult<UserRole> {
        UserRole::try_from(self.role)
    }

    pub fn partition_size(&self) -> &Option<u64> {
        &self.partition_size
    }
}

#[derive(Serialize)]
pub struct UserSerializer {
    id: String,
    email: Option<String>,
    role: UserRole,
    partition: Option<String>,
    partition_size: Option<u64>,
    created_at: String,
}

impl IntoSerializer for Users {
    type Serializer = UserSerializer;

    async fn into_serializer(self, _: &RBatis) -> CoreResult<Self::Serializer> {
        // let role = &self.role();
        let Users {
            pid: id,
            email,
            partition,
            partition_size,
            created_at,
            role,
            ..
        } = self;

        let role = role.try_into()?;
        Ok(UserSerializer {
            id,
            email,
            role,
            partition,
            partition_size,
            created_at: created_at.to_string(),
        })
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub enum UserRole {
    /// can only read assets
    Basic,

    /// full asset management
    Manager,

    /// full application management
    Admin,
}

impl TryFrom<u8> for UserRole {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use UserRole::*;

        if value == 0 {
            Ok(Basic)
        } else if value == 1 {
            Ok(Manager)
        } else if value == 2 {
            Ok(Admin)
        } else {
            Err(CoreError::ParseError(format!(
                "invalid user_role '{value}' "
            )))
        }
    }
}

impl From<UserRole> for u8 {
    fn from(value: UserRole) -> Self {
        use UserRole::*;

        match value {
            Basic => 0,
            Manager => 1,
            Admin => 2,
        }
    }
}
