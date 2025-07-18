use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select, rbdc::DateTime};
use rbs::value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CoreResult, errors::CoreError};

use super::{IntoSerializer, asset::Assets, check_model, permission::AssetPermissions};

#[derive(Serialize, Deserialize, Modeller)]
pub struct Users {
    #[modeller(serial)]
    id: Option<u64>,

    #[modeller(unique)]
    pid: String,
    role: u8,
    client_id: Option<u64>,
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

    pub async fn create_by_client(rb: &RBatis, client_id: u64) -> CoreResult<String> {
        let role: u8 = UserRole::General.into();
        let pid = Uuid::new_v4().to_string();
        let created_at = DateTime::now();

        let user = Users {
            id: None,
            pid,
            role,
            email: None,
            password: None,
            client_id: Some(client_id),
            created_at,
        };

        Users::insert(rb, &user).await?;
        Ok(user.pid)
    }

    pub async fn delete(&self, rb: &RBatis) -> CoreResult<()> {
        Users::delete_by_map(
            rb,
            value! {
                "id": &self.id
            },
        )
        .await?;

        self.clean_up(rb).await?;

        Ok(())
    }

    /// Removes user permissions and assets. To be called inside or after [User::delete].
    async fn clean_up(&self, rb: &RBatis) -> CoreResult<()> {
        AssetPermissions::delete_for_user(rb, &self.id()).await?;
        Assets::delete_for_user(rb, &self.id(), true).await?;
        Ok(())
    }

    pub fn id(&self) -> u64 {
        *&self.id.unwrap_or_default()
    }

    pub fn role(&self) -> CoreResult<UserRole> {
        UserRole::try_from(self.role)
    }
}

#[derive(Serialize)]
pub struct UserSerializer {
    id: String,
    email: Option<String>,
    role: UserRole,
    created_at: String,
}

impl IntoSerializer for Users {
    type Serializer = UserSerializer;

    async fn into_serializer(self, _: &RBatis) -> CoreResult<Self::Serializer> {
        // let role = &self.role();
        let Users {
            pid: id,
            email,
            created_at,
            role,
            ..
        } = self;

        let role = role.try_into()?;
        Ok(UserSerializer {
            id,
            email,
            role,
            created_at: created_at.to_string(),
        })
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub enum UserRole {
    General,
    Admin,
}

impl TryFrom<u8> for UserRole {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use UserRole::*;

        if value == 0 {
            Ok(General)
        } else if value == 1 {
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
            General => 0,
            Admin => 1,
        }
    }
}
