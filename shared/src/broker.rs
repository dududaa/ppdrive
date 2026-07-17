use anyhow::anyhow;
use redis::AsyncCommands;
use crate::server::UploadInfo;

type RedisConnection = redis::aio::MultiplexedConnection;

#[derive(Clone)]
pub struct MessageBroker {
    conn: RedisConnection
}

impl MessageBroker {
    pub async fn new(url: &str) -> anyhow::Result<MessageBroker> {
        let client = redis::Client::open(url)?;
        let conn = client.get_multiplexed_async_connection().await?;
        
        Ok(Self { conn })
    }
    
    fn conn(&self) -> RedisConnection {
        self.conn.clone()
    }
    
    pub async fn get_upload_info(&self, session_id: &str) -> anyhow::Result<UploadInfo> {
        let data = self.conn().get::<_, String>(session_id).await.map_err(|e| anyhow!("{e}"))?;
        let info = serde_json::from_str(&data)?;
        
        Ok(info)
    }
    
    pub async fn upsert_upload_info(&self, session_id: &str, info: &UploadInfo) -> anyhow::Result<()> {
        let data = serde_json::to_string(info)?;
        self.conn().set_ex::<_, String, u64>(session_id, data, info.chunk_session_expiration as u64).await.map_err(|e| anyhow!("{e}"))?;
        
        Ok(())
    }
    
    pub async fn remove_upload_info(&self, session_id: &str) -> anyhow::Result<()> {
        self.conn().del::<_, String>(session_id).await.map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }
}