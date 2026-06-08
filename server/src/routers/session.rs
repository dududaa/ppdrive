use crate::state::AppState;
use crate::utils::{Claims, ClaimsData, create_jwt};
use shared::generate_nano_id;
use shared::sqlx_qb::prelude::*;

const TABLE_NAME: &str = "sessions";

pub(super) async fn create_session(state: &AppState) -> anyhow::Result<String> {
    let pid = generate_nano_id(24);
    sqlx::query("INSERT INTO sessions (pid) VALUES ($1)")
        .bind(&pid)
        .execute(state.pool())
        .await?;

    Ok(pid)
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

pub(super) fn next_session_token(
    state: &AppState,
    client_id: i32,
    data: ClaimsData,
) -> anyhow::Result<String> {
    // TODO: Make resumable expiration configurable.
    let claims = Claims::new(client_id, 30, data)?.with_session_resume(true);
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
