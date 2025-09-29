use crate::{
    AppResult, Manager,
    ops::{list_services, start_service},
};
use anyhow::anyhow;
use handlers::plugin::service::Service;
use ppd_shared::opts::ServiceConfig;

#[tokio::test]
async fn test_start_and_stop_manager() -> AppResult<()> {
    let manager = Manager::default();
    let handle = manager.start_background().await;

    // check list of running services to be sure manager is running
    let mut socket = manager.tcp_stream().await?;
    let shared = manager.shared();
    let check = list_services(shared, &mut socket).await;
    assert!(check.is_ok());

    manager.close().await;
    let res = handle.await;

    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_start_service() -> AppResult<()> {
    let manager = Manager::default();
    let mut config = ServiceConfig::default();
    config.auto_install = true;

    // caller is responsible for initializing the service before before sending a
    // request to start the service
    let svc = Service::from(&config);
    svc.init().map_err(|err| anyhow!(err))?;

    // let's start the service
    let shared = manager.shared();
    let handle = manager.start_background().await;
    let mut socket = manager.tcp_stream().await?;

    let start = start_service(shared, config, &mut socket).await;
    assert!(start.is_ok());
    manager.close().await;

    let res = handle.await?;
    assert!(res.is_ok());

    Ok(())
}
