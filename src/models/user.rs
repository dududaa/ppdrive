use crate::{
    errors::AppError, models::PermissionGroup, routes::admin::CreateUserRequest, state::DbPooled,
};
use chrono::NaiveDateTime;
use diesel::{
    prelude::{Associations, Identifiable, Insertable, Queryable, Selectable},
    ExpressionMethods, QueryDsl, SelectableHelper,
};
use diesel_async::RunQueryDsl;
use serde::Serialize;

use super::{TryFromModel, Permission};

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
    pub async fn get(conn: &mut DbPooled<'_>, user_id: i32) -> Result<Self, AppError> {
        use crate::schema::users::dsl::*;

        let user = users
            .find(user_id)
            .select(User::as_select())
            .first(conn)
            .await
            .map_err(|err| AppError::InternalServerError(err.to_string()))?;

        Ok(user)
    }

    pub async fn create(conn: &mut DbPooled<'_>, data: CreateUserRequest) -> Result<i32, AppError> {
        use crate::schema::users::dsl::users;
        use crate::schema::users::*;

        let pg: i16 = data.permission_group.clone().into();

        let user = diesel::insert_into(users)
            .values(permission_group.eq(pg))
            .returning(User::as_returning())
            .get_result(conn)
            .await
            .map_err(|err| AppError::DatabaseError(err.to_string()))?;

        if let PermissionGroup::Custom = data.permission_group {
            if let Some(perms) = data.permissions {
                for perm in perms {
                    UserPermission::create(conn, user.id, perm).await?;
                }
            }
        }

        Ok(user.id)
    }

    pub async fn delete(conn: &mut DbPooled<'_>, user_id: i32) -> Result<(), AppError> {
        use crate::schema::users::dsl::*;

        diesel::delete(users.filter(id.eq(user_id)))
            .execute(conn)
            .await
            .map_err(|err| AppError::DatabaseError(err.to_string()))?;

        Ok(())
    }

    pub async fn permissions(&self, conn: &mut DbPooled<'_>) -> Result<Option<Vec<Permission>>, AppError> {
        use crate::schema::user_permissions::dsl::*;

        let pg = PermissionGroup::try_from(self.permission_group)?;
        let perms = match pg {
            PermissionGroup::Custom => {
                let user_perms = user_permissions
                    .filter(user_id.eq(self.id))
                    .load::<UserPermission>(conn)
                    .await
                    .map_err(|err| AppError::DatabaseError(err.to_string()))?;
        
                let mut perms = Vec::with_capacity(user_perms.len());
                for perm in user_perms {
                    perms.push(perm.try_into()?);
                }

                Some(perms)
            }
            _ => pg.default_permissions()
        };

        Ok(perms)
    }
}

#[derive(Queryable, Selectable, Identifiable, Associations)]
#[diesel(belongs_to(User))]
#[diesel(table_name = crate::schema::user_permissions)]
pub struct UserPermission {
    pub id: i32,
    pub user_id: i32,
    pub permission: i16,
}

impl UserPermission {
    async fn create(conn: &mut DbPooled<'_>, uid: i32, perm: Permission) -> Result<(), AppError> {
        use crate::schema::user_permissions::dsl::*;
        let val: i16 = perm.into();

        diesel::insert_into(user_permissions)
            .values((user_id.eq(uid), permission.eq(val)))
            .execute(conn)
            .await
            .map_err(|err| AppError::DatabaseError(err.to_string()))?;

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
    pub is_admin: bool,
    pub permission_group: PermissionGroup,
    pub permissions: Option<Vec<Permission>>,
    pub created_at: String,
}

impl TryFromModel<User> for UserSerializer {
    type Error = AppError;
    
    async fn try_from_model(conn: &mut DbPooled<'_>, model: User) -> Result<Self, Self::Error> {
        let User { is_admin, permission_group, created_at, .. } = model;

        let permission_group = PermissionGroup::try_from(permission_group)?;
        let permissions = model.permissions(conn).await?;

        Ok(Self { is_admin, permission_group, permissions, created_at: created_at.to_string() })
    }
}