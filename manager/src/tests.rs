use std::time::Duration;
use ppd_shared::opts::ServiceConfig;
use tokio::{net::TcpStream, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{AppResult, ServiceManager, DEFAULT_PORT};

#[tokio::test]
async fn test_start_and_stop_manager() {
    let manager = ServiceManager::test();
    let token = manager.token.clone();
    let handle = tokio::spawn(async move { manager.start().await });

    tokio::time::sleep(Duration::from_secs(15)).await;
    token.cancel();
    let res = handle.await;


    assert!(res.is_ok())
}

#[tokio::test]
async fn test_start_service() -> AppResult<()> {
    let manager = ServiceManager::test();
    let token = manager.token.clone();
    
    let config = ServiceConfig::default();
    let mut socket = TcpStream::connect(manager.addr().clone()).await?;
    
    let handle = tokio::spawn(async move { manager.start().await });

    Ok(())
}