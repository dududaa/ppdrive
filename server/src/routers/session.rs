use crate::state::AppState;
use crate::utils::{Claims, ClaimsData, create_jwt};
use shared::generate_nano_id;
use sqlx::types::time::OffsetDateTime;

pub(super) async fn create_session(state: &AppState) -> anyhow::Result<String> {
    let pid = generate_nano_id(24);
    let now = OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?;

    let db = state.db();
    let query = format!(
        "INSERT INTO sessions (pid, created_at) VALUES ({}, {})",
        db.placeholder(1),
        db.placeholder(2)
    );
    sqlx::query(sqlx::AssertSqlSafe(query.as_str()))
        .bind(&pid)
        .bind(&now)
        .execute(state.pool())
        .await?;

    Ok(pid)
}

pub(super) async fn check_session(state: &AppState, pid: &str) -> anyhow::Result<bool> {
    let query = format!("SELECT used FROM sessions WHERE pid = {} LIMIT 1", state.db().placeholder(1));
    let used = sqlx::query_scalar(sqlx::AssertSqlSafe(query.as_str()))
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
    sqlx::query("UPDATE sessions SET used = $1 WHERE pid = $2")
        .bind(true)
        .bind(pid)
        .execute(state.pool())
        .await?;
    Ok(())
}
