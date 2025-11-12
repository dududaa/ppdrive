use crate::{
    AppResult, Manager,
    ops::{list_services, start_service, stop_service},
};
use anyhow::anyhow;
use ppd_shared::opts::internal::ServiceConfig;
use ppdrive::plugin::service::Service;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_start_and_stop_manager() -> AppResult<()> {
    let manager = Manager::default();
    let handle = manager.start_background().await;

    // check list of running services to be sure manager is running
    let mut socket = manager.connect().await?;
    let shared = manager.shared();
    let check = list_services(shared, &mut socket).await;
    assert!(check.is_ok());

    manager.close().await;
    let res = handle.await;

    assert!(res.is_ok());
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_start_service() -> AppResult<()> {
    let manager = Manager::default();
    let mut config = ServiceConfig::default();
    config.auto_install = true;

    // client is responsible for initializing the service before before sending a
    // request to start the service
    let svc = Service::from(&config);
    svc.init().map_err(|err| anyhow!(err))?;

    // let's start the service
    let shared = manager.shared();
    let handle = manager.start_background().await;
    let mut socket = manager.connect().await?;

    // let svc_url = svc.addr();
    // let db_url = config.base.db_url.clone();
    let start = start_service(shared, config, &mut socket).await;
    assert!(start.is_ok());

    // send test requets
    // let resp = send_test_request(&svc_url, &db_url).await?;
    // let body = resp.text().await.map_err(|err| anyhow!(err))?;
    // println!("{body}");

    manager.close().await;

    let res = handle.await?;
    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_stop_service() -> AppResult<()> {
    let manager = Manager::default();
    let mut config = ServiceConfig::default();
    config.auto_install = true;

    // client is responsible for initializing the service before before sending a
    // request to start the service
    let svc = Service::from(&config);
    svc.init().map_err(|err| anyhow!(err))?;

    // let's start the service
    let shared = manager.shared();
    let handle = manager.start_background().await;
    let mut socket = manager.connect().await?;

    let id = start_service(shared.clone(), config, &mut socket).await?;
    let stop = stop_service(shared, id, &mut socket).await;
    assert!(stop.is_ok());

    manager.close().await;
    let _ = handle.await?;

    Ok(())
}
