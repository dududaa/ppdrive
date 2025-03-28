use serde::Deserialize;

pub mod user;

#[derive(Deserialize)]
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

#[derive(Deserialize, Clone)]
pub enum PermissionGroup {
    Full,
    Read,
    Write,
    Delete,
    Custom,
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
        }
    }
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
