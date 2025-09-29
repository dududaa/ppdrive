use ppd_shared::opts::ServiceConfig;
use crate::{
    AppResult, Manager,
    ops::{list_services, start_service},
};

#[tokio::test]
async fn test_start_and_stop_manager() -> AppResult<()> {
    let manager = Manager::default();
    let handle = manager.start_background().await;

    // check list of running services to be sure manager is running
    let mut socket = manager.tcp_stream().await?;
    let shared = manager.shared();
    let check = list_services(shared, &mut socket).await;
    assert!(check.is_ok());

    manager.close();
    let res = handle.await;

    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_start_service() -> AppResult<()> {
    let manager = Manager::default();
    let config = ServiceConfig::default();
    let shared = manager.shared();

    let handle = manager.start_background().await;
    let mut socket = manager.tcp_stream().await?;

    let start = start_service(shared, config, &mut socket).await;
    assert!(start.is_ok());
    manager.close();

    let _ = handle.await?;
    Ok(())
}
