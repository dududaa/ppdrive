use crate::state::AppState;
use crate::utils::{Claims, ClaimsData, create_jwt};
use shared::sqlx_qb::prelude::*;

const TABLE_NAME: &'static str = "sessions";

pub(super) async fn create_session_id(state: &AppState, pid: &str) -> anyhow::Result<String> {
    let map = query_map! {
        "pid": &pid,
    };

    let id = QB::new(state.pool())
        .with_table_name(TABLE_NAME)
        .insert_returns(&map, "pid")
        .await?;

    Ok(id)
}

pub(super) async fn check_session(state: &AppState, pid: &str) -> anyhow::Result<bool> {
    let modifiers = Modifiers::new().with_filter(("pid", pid)).with_limit(1);
    let used = QB::new(state.pool())
        .with_table_name(TABLE_NAME)
        .with_modifiers(&modifiers)
        .select_scalar("used")
        .await?;

    Ok(used)
}

pub(super) async fn next_token(
    state: &AppState,
    client_id: i32,
    data: ClaimsData,
) -> anyhow::Result<String> {
    let claims = Claims {
        sub: client_id,
        exp: 30,
        data,
    };

    create_jwt(state.secrets(), &claims)
}

pub(crate) async fn revoke_token(state: &AppState, pid: &str) -> anyhow::Result<()> {
    let modifiers = Modifiers::new().with_filter(("pid", pid));
    let map = query_map! { "used": true };

    QB::new(state.pool())
        .with_table_name(TABLE_NAME)
        .with_modifiers(&modifiers)
        .update(&map)
        .await?;

    Ok(())
}
