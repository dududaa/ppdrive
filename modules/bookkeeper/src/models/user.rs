use modeller::prelude::*;
use rbatis::{RBatis, crud, impl_select, rbdc::DateTime};
use rbs::value;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{errors::Error as DBError, DBResult};

use super::{IntoSerializer, asset::Assets, check_model, permission::AssetPermissions};

#[derive(Serialize, Deserialize, Modeller)]
pub struct Users {
    id: Option<u64>,

    #[modeller(unique)]
    pid: String,
    role: u8,
    client_id: Option<u64>,
    username: Option<String>,
    password: Option<String>,

    /// maximum accumulated size of buckets user can create
    max_bucket: Option<f64>,
    created_at: DateTime,
}

crud!(Users {});
impl_select!(Users { get_by_key<V: Serialize>(key: &str, value: V) -> Option => "`WHERE ${key} = #{value} LIMIT 1`" });
impl_select!(Users { get_for_client(id: &str, client_id: &u64) -> Option => "`WHERE pid = #{id} AND client_id = #{client_id} LIMIT 1`" });

impl Users {
    pub async fn get(rb: &RBatis, user_id: &u64) -> DBResult<Users> {
        let user = Users::get_by_key(rb, "id", user_id).await?;
        check_model(user, "user not found")
    }

    pub async fn get_by_pid(rb: &RBatis, pid: &str) -> DBResult<Users> {
        let user = Users::get_by_key(rb, "pid", pid).await?;
        check_model(user, "user not found")
    }

    pub async fn get_by_partition_name(rb: &RBatis, partition_name: &str) -> DBResult<Users> {
        let user = Users::get_by_key(rb, "partition", partition_name).await?;
        check_model(user, "user not found")
    }

    pub async fn create_by_client(
        db: &RBatis,
        client_id: u64,
        bucket_size: Option<f64>,
    ) -> DBResult<String> {
        let role: u8 = UserRole::General.into();
        let pid = Uuid::new_v4().to_string();
        let created_at = DateTime::now();

        let user = Users {
            id: None,
            pid,
            role,
            username: None,
            password: None,
            client_id: Some(client_id),
            max_bucket: bucket_size,
            created_at,
        };

        Users::insert(db, &user).await?;
        Ok(user.pid)
    }

    pub async fn create(db: &RBatis, username: String, password: String) -> DBResult<String> {
        let pid = Uuid::new_v4().to_string();
        let role: u8 = UserRole::General.into();
        let created_at = DateTime::now();
        
        let user = Users {
            id: None,
            username: Some(username),
            password: Some(password),
            pid,
            role,
            client_id: None,
            max_bucket: None,
            created_at
        };

        Users::insert(db, &user).await?;
        Ok(user.pid)
    }

    pub async fn delete(&self, rb: &RBatis) -> DBResult<()> {
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
    async fn clean_up(&self, rb: &RBatis) -> DBResult<()> {
        AssetPermissions::delete_for_user(rb, &self.id()).await?;
        Assets::delete_for_user(rb, &self.id()).await?;
        Ok(())
    }

    pub fn id(&self) -> u64 {
        self.id.unwrap_or_default()
    }

    pub fn role(&self) -> DBResult<UserRole> {
        UserRole::try_from(self.role)
    }

    pub fn client_id(&self) -> &Option<u64> {
        &self.client_id
    }

    pub fn password(&self) -> &Option<String> {
        &self.password
    }

    pub fn max_bucket_size(&self) -> &Option<f64> {
        &self.max_bucket
    }
}

#[derive(Serialize)]
pub struct UserSerializer {
    id: String,
    email: Option<String>,
    role: UserRole,
    created_at: String,
    max_bucket: Option<f64>,
}

impl IntoSerializer for Users {
    type Serializer = UserSerializer;

    async fn into_serializer(self, _: &RBatis) -> DBResult<Self::Serializer> {
        // let role = &self.role();
        let Users {
            pid: id,
            username: email,
            created_at,
            role,
            max_bucket,
            ..
        } = self;

        let role = role.try_into()?;
        Ok(UserSerializer {
            id,
            email,
            role,
            max_bucket,
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
    type Error = DBError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use UserRole::*;

        if value == 0 {
            Ok(General)
        } else if value == 1 {
            Ok(Admin)
        } else {
            Err(DBError::ParseError(format!("invalid user_role '{value}' ")))
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