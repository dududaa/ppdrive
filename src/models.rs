use chrono::NaiveDateTime;
use diesel::prelude::{Insertable, Queryable, Selectable};

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
    DeleteFolder
}

pub enum PermissionGroup {
    Full,
    Read,
    Write,
    Delete,
    Custom
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub is_admin: bool,
    pub permission_group: i16,
    pub created_at: NaiveDateTime,
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