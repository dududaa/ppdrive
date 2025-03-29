use diesel::{ExpressionMethods, SelectableHelper};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::{
    errors::AppError,
    models::{user::User, PermissionGroup},
    state::create_db_pool,
};

pub fn get_env(key: &str) -> Result<String, AppError> {
    std::env::var(key).map_err(|err| {
        tracing::error!("unable to get var {key}: {err}");
        err.into()
    })
}

pub async fn create_admin() -> Result<Uuid, AppError> {
    use crate::schema::users::dsl::users;
    use crate::schema::users::*;

    let pool = create_db_pool().await?;
    let mut conn = pool.get().await?;
    let pg: i16 = PermissionGroup::Full.into();

    let admin = diesel::insert_into(users)
        .values((is_admin.eq(true), permission_group.eq(pg)))
        .returning(User::as_returning())
        .get_result(&mut conn)
        .await
        .map_err(|err| AppError::DatabaseError(err.to_string()))?;

    Ok(admin.pid)
}
