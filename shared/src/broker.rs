//! Redis-backed message broker for resumable upload sessions.
//!
//! Stores [`UploadInfo`] payloads keyed by session ID with automatic TTL expiration.

use anyhow::anyhow;
use redis::{AsyncCommands, Value};
use crate::server::UploadInfo;

type RedisConnection = redis::aio::MultiplexedConnection;

#[derive(Clone)]
pub struct MessageBroker {
    conn: RedisConnection
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
        let data = self.conn().get::<_, String>(session_id).await.map_err(|e| anyhow!("{e}"))?;
        let info = serde_json::from_str(&data)?;
        
        Ok(info)
    }
    
    /// Insert or update an [`UploadInfo`] with TTL set to its expiration time.
    pub async fn upsert_upload_info(&self, session_id: &str, info: &UploadInfo) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| anyhow!("{e}"))?
            .as_secs() as i64;
        let ttl = (info.exp - now).max(0) as u64;

        let data = serde_json::to_string(info)?;
        self.conn().set_ex::<_, String, Value>(session_id, data, ttl).await.map_err(|e| anyhow!("{e}"))?;
        
        Ok(())
    }
    
    /// Delete an upload session from the broker.
    pub async fn remove_upload_info(&self, session_id: &str) -> anyhow::Result<()> {
        self.conn().del::<_, String>(session_id).await.map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }
}