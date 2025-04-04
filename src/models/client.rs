use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, Selectable, SelectableHelper};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::{errors::AppError, state::DbPooled};

pub struct CreateClientOpts {
    pub key: Vec<u8>,
    pub payload: Vec<u8>
}

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = crate::schema::clients)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Client {
    pub id: i32,
    pub enc_key: Vec<u8>,
    pub enc_payload: Vec<u8>,
    pub cid: Uuid
}

impl Client {
    pub async fn get(conn: &mut DbPooled<'_>, cuid: Uuid) -> Result<Self, AppError> {
        use crate::schema::clients::dsl::*;

        clients
            .filter(cid.eq(cuid))
            .select(Client::as_select())
            .first(conn)
            .await
            .map_err(|err| AppError::InternalServerError(err.to_string()))

    }

    pub async fn create(conn: &mut DbPooled<'_>, opts: CreateClientOpts) -> Result<Self, AppError> {
        use crate::schema::clients::dsl::*;

        let client = diesel::insert_into(clients)
            .values((
                enc_payload.eq(opts.payload),
                enc_key.eq(opts.key)
            ))
            .returning(Client::as_returning())
            .get_result(conn)
            .await
            .map_err(|err| AppError::DatabaseError(err.to_string()))?;

        Ok(client)
    }
}