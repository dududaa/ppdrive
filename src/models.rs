use chrono::NaiveDateTime;
use diesel::{
    prelude::{Insertable, Queryable, Selectable},
    query_dsl::methods::{FindDsl, SelectDsl},
    ExpressionMethods, SelectableHelper,
};
use diesel_async::RunQueryDsl;
use serde::Deserialize;

use crate::{errors::PPDriveError, routes::admin::CreateUserRequest, state::DbPooled};

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

#[derive(Deserialize)]
pub enum PermissionGroup {
    Full,
    Read,
    Write,
    Delete,
    Custom,
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

impl User {
    pub async fn get(conn: &mut DbPooled<'_>, user_id: i32) -> Result<User, PPDriveError> {
        use crate::schema::users::dsl::*;

        let user = users
            .find(user_id)
            .select(User::as_select())
            .first(conn)
            .await
            .map_err(|err| PPDriveError::InternalServerError(err.to_string()))?;

        Ok(user)
    }

    pub async fn create(
        conn: &mut DbPooled<'_>,
        data: CreateUserRequest,
    ) -> Result<i32, PPDriveError> {
        use crate::schema::users::dsl::users;
        use crate::schema::users::*;

        // let CreateUserRequest { permission_group, permissions } = data;
        let pg: i16 = data.permission_group.into();

        let user = diesel::insert_into(users)
            .values((permission_group.eq(pg)))
            .returning(User::as_returning())
            .get_result(conn)
            .await
            .map_err(|err| PPDriveError::DatabaseError(err.to_string()))?;

        Ok(user.id)
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
