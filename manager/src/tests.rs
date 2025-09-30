use crate::{
    ops::{create_new_client, list_services, start_service, stop_service}, AppResult, Manager
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

    // client is responsible for initializing the service before before sending a
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


#[tokio::test]
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
    let mut socket = manager.tcp_stream().await?;

    let id = start_service(shared.clone(), config, &mut socket).await?;
    let stop = stop_service(shared, id, &mut socket).await;
    assert!(stop.is_ok());
    
    manager.close().await;
    let _ = handle.await?;

    Ok(())
}

#[tokio::test]
async fn test_create_client() -> AppResult<()> {
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
    let mut socket = manager.tcp_stream().await?;

    println!("starting service...");
    let id = start_service(shared.clone(), config, &mut socket).await?;

    println!("creating token for {id}...");
    let token = create_new_client(shared, id, "Test Client".to_string()).await;
    
    println!("create client complete...");
    assert!(token.is_ok());
    
    manager.close().await;
    let _ = handle.await?;

    Ok(())
}