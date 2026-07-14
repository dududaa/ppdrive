use sqlx::types::time::OffsetDateTime;
use crate::state::AppState;
use crate::utils::{Claims, ClaimsData, create_jwt};
use shared::generate_nano_id;

pub(super) async fn create_session(state: &AppState) -> anyhow::Result<String> {
    let pid = generate_nano_id(24);
    let now = OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?;

    sqlx::query("INSERT INTO sessions (pid, created_at) VALUES ($1, $2)")
        .bind(&pid)
        .bind(&now)
        .execute(state.pool())
        .await?;

    Ok(pid)
}

pub(super) async fn check_session(state: &AppState, pid: &str) -> anyhow::Result<bool> {
    let used = sqlx::query_scalar("SELECT used FROM sessions WHERE pid = $1 LIMIT 1")
        .bind(pid)
        .fetch_one(state.pool())
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
    sqlx::query("UPDATE sessions SET used = $1 WHERE pid = $2").bind(true).bind(pid).execute(state.pool()).await?;
    Ok(())
}
