use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(FromRow)]
pub struct Asset {
    pub id: i32,
    pub pid: String,
    pub bucket_id: i32,
    pub path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i16)]
pub enum PermissionLevel {
    Read = 0,
    Write = 1,
    Admin = 2,
}

impl From<i16> for PermissionLevel {
    fn from(value: i16) -> Self {
        match value {
            0 => PermissionLevel::Read,
            1 => PermissionLevel::Write,
            2 => PermissionLevel::Admin,
            _ => {
                tracing::warn!("unknown permission level: {value}, defaulting to Read");
                PermissionLevel::Read
            }
        }
    }
}

impl From<PermissionLevel> for i16 {
    fn from(value: PermissionLevel) -> Self {
        value as i16
    }
}

impl std::fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionLevel::Read => write!(f, "read"),
            PermissionLevel::Write => write!(f, "write"),
            PermissionLevel::Admin => write!(f, "admin"),
        }
    }
}

impl std::str::FromStr for PermissionLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "read" => Ok(PermissionLevel::Read),
            "write" => Ok(PermissionLevel::Write),
            "admin" => Ok(PermissionLevel::Admin),
            _ => Err(anyhow::anyhow!("invalid permission level: {s}")),
        }
    }
}

#[derive(FromRow, Serialize, Deserialize)]
pub struct FilePermission {
    pub id: i32,
    pub asset_id: i32,
    pub grantee_id: i32,
    pub permission: i16,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct PermissionWithGrantee {
    pub id: i32,
    pub asset_id: i32,
    pub grantee_id: i32,
    pub grantee_type: String,
    pub grantee_name: String,
    pub permission: String,
    pub created_at: String,
}
