use serde::{Deserialize, Serialize};
use crate::{errors::AppError, state::DbPooled};

pub mod user;

pub trait TryFromModel<M>: Sized {
    type Error;
    async fn try_from_model(conn: &mut DbPooled<'_>, model: M) -> Result<Self, Self::Error>;
}

#[derive(Deserialize, Serialize)]
pub enum Permission {
    // write
    CreateFile,
    CreateFolder,
    RenameFolder,
    RenameFile,
    ReplaceFile,

    // read
    ReadFile,
    ReadFolder,

    // delete
    DeleteFile,
    DeleteFolder,
}


impl From<Permission> for i16 {
    fn from(value: Permission) -> Self {
        match value {
            Permission::CreateFile => 0,
            Permission::CreateFolder => 1,
            Permission::ReadFile => 2,
            Permission::ReadFolder => 3,
            Permission::RenameFile => 4,
            Permission::RenameFolder => 5,
            Permission::ReplaceFile => 6,
            Permission::DeleteFile => 7,
            Permission::DeleteFolder => 8,
        }
    }
}

impl TryFrom<i16> for Permission {
    type Error = AppError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Permission::CreateFile),
            1 => Ok(Permission::CreateFolder),
            2 => Ok(Permission::ReadFile),
            3 => Ok(Permission::ReadFolder),
            4 => Ok(Permission::RenameFile),
            5 => Ok(Permission::RenameFolder),
            6 => Ok(Permission::ReplaceFile),
            7 => Ok(Permission::DeleteFile),
            8 => Ok(Permission::DeleteFolder),
            _ => Err(AppError::ParsingError(format!("'{value}' is invalid permission."))),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub enum PermissionGroup {
    Full,
    Read,
    Write,
    Delete,
    Custom,
    Null,
}

impl PermissionGroup {
    pub fn default_permissions(&self) -> Option<Vec<Permission>> {
        match self {
            PermissionGroup::Read => Some(vec![Permission::ReadFile, Permission::ReadFolder]),
            PermissionGroup::Write => Some(vec![
                Permission::CreateFile,
                Permission::CreateFolder,
                Permission::RenameFile,
                Permission::RenameFolder,
                Permission::ReplaceFile,
            ]),
            PermissionGroup::Delete => Some(vec![Permission::DeleteFile, Permission::DeleteFolder]),
            _ => None
        }
    }
}

impl From<PermissionGroup> for i16 {
    fn from(value: PermissionGroup) -> Self {
        match value {
            PermissionGroup::Full => 0,
            PermissionGroup::Read => 1,
            PermissionGroup::Write => 2,
            PermissionGroup::Delete => 3,
            PermissionGroup::Custom => 4,
            PermissionGroup::Null => 5,
        }
    }
}

impl TryFrom<i16> for PermissionGroup {
    type Error = AppError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PermissionGroup::Full),
            1 => Ok(PermissionGroup::Read),
            2 => Ok(PermissionGroup::Write),
            3 => Ok(PermissionGroup::Delete),
            4 => Ok(PermissionGroup::Custom),
            5 => Ok(PermissionGroup::Null),
            _ => Err(AppError::ParsingError(format!("'{value}' is invalid permission group"))),
        }
    }
}


