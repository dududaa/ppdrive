use std::collections::HashMap;

use crate::errors::ServerError;
use axum::{
    Json,
    extract::{Multipart, Query, State},
};
use axum_macros::debug_handler;
use ppd_bk::models::{
    IntoSerializer,
    bucket::Buckets,
    user::{UserSerializer, Users},
};
use ppd_shared::opts::{OptionValidator, api::CreateBucketOptions};
use ppdrive::{
    prelude::state::HandlerState,
    rest::{
        create_asset_user, delete_asset_user,
        extractors::{BucketSizeValidator, ClientUserExtractor},
    },
};

#[debug_handler]
pub async fn get_user(
    State(state): State<HandlerState>,
    user: ClientUserExtractor,
) -> Result<Json<UserSerializer>, ServerError> {
    let db = state.db();
    let user_model = Users::get(db, user.id()).await?;
    let data = user_model.into_serializer(db).await?;

    Ok(Json(data))
}

#[debug_handler]
pub async fn create_user_bucket(
    State(state): State<HandlerState>,
    user: ClientUserExtractor,
    Json(data): Json<CreateBucketOptions>,
) -> Result<String, ServerError> {
    data.validate_data()?;
    let db = state.db();

    user.validate_bucket_size(db, &data.size).await?;
    let id = Buckets::create_by_user(db, data, *user.id()).await?;

    Ok(id)
}

#[debug_handler]
pub async fn create_asset(
    State(state): State<HandlerState>,
    user: ClientUserExtractor,
    multipart: Multipart,
) -> Result<String, ServerError> {
    let path = create_asset_user(user.id(), multipart, state).await?;
    Ok(path)
}

#[debug_handler]
pub async fn delete_asset(
    Query(query): Query<HashMap<String, String>>,
    State(state): State<HandlerState>,
    user: ClientUserExtractor,
) -> Result<String, ServerError> {
    delete_asset_user(user.id(), query, state).await?;
    Ok("operation successful".to_string())
}
