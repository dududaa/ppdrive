//! Redis-backed message broker for resumable upload sessions.
//!
//! Stores [`UploadInfo`] payloads keyed by session ID with automatic TTL expiration.

use crate::server::UploadInfo;
use anyhow::anyhow;
use redis::{AsyncCommands, Value};

type RedisConnection = redis::aio::MultiplexedConnection;

#[derive(Clone)]
pub struct MessageBroker {
    conn: RedisConnection,
}

impl MessageBroker {
    /// Connect to Redis and return a new [`MessageBroker`].
    pub async fn new(url: &str) -> anyhow::Result<MessageBroker> {
        let client = redis::Client::open(url)?;
        let conn = client.get_multiplexed_async_connection().await?;

        Ok(Self { conn })
    }

    fn conn(&self) -> RedisConnection {
        self.conn.clone()
    }

    /// Retrieve a previously stored [`UploadInfo`] by session ID.
    pub async fn get_upload_info(&self, session_id: &str) -> anyhow::Result<UploadInfo> {
        let data = self
            .conn()
            .get::<_, String>(session_id)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        let info = serde_json::from_str(&data)?;

        Ok(info)
    }

    /// Insert or update an [`UploadInfo`] with TTL set to its expiration time.
    pub async fn upsert_upload_info(
        &self,
        session_id: &str,
        info: &UploadInfo,
    ) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| anyhow!("{e}"))?
            .as_secs() as i64;
        let ttl = (info.exp - now).max(0) as u64;

        let data = serde_json::to_string(info)?;
        self.conn()
            .set_ex::<_, String, Value>(session_id, data, ttl)
            .await
            .map_err(|e| anyhow!("{e}"))?;

        Ok(())
    }

    /// Delete an upload session from the broker.
    pub async fn remove_upload_info(&self, session_id: &str) -> anyhow::Result<()> {
        self.conn()
            .del::<_, String>(session_id)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    /// Fetch a cached transformed-download payload by cache key.
    ///
    /// Returns `Ok(None)` when the entry is absent (expired or never stored).
    pub async fn get_transform_cache(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let value = self
            .conn()
            .get::<_, Option<Vec<u8>>>(transform_cache_key(key))
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(value)
    }

    /// Store a transformed-download payload, expiring after `ttl_secs`.
    ///
    /// A `ttl_secs` of 0 is a no-op (the cache is disabled).
    pub async fn set_transform_cache(
        &self,
        key: &str,
        value: &[u8],
        ttl_secs: u64,
    ) -> anyhow::Result<()> {
        if ttl_secs == 0 {
            return Ok(());
        }
        self.conn()
            .set_ex::<_, &[u8], Value>(transform_cache_key(key), value, ttl_secs)
            .await
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }
}

/// Namespaced Redis key for a transformed-download cache entry, kept distinct
/// from raw upload session ids.
fn transform_cache_key(key: &str) -> String {
    format!("ppdrive:xform:{key}")
}
