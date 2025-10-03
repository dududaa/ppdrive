use std::{net::TcpStream, thread::sleep, time::Duration};

use ppd_shared::opts::ServiceConfig;

use crate::{command::start_manager, errors::AppResult, imp::PPDrive};

#[test]
fn test_create_client() -> AppResult<()> {
    // start manager
    let port = 5025;
    let port_clone = port.clone();
    start_manager(port_clone, Some("../")).expect("cannot start manager");

    // wait till we're able to establish connection with manager
    let mut retry = 1;
    let max_try = 24; // you can increase max_try if manager takes longer to load
    let addr = format!("0.0.0.0:{}", port);

    loop {
        println!("{retry}/{max_try} connecting to manager at {addr}...\n");
        sleep(Duration::from_secs(5));
        let server_ready = TcpStream::connect(&addr);

        match server_ready {
            Ok(_) => break,
            Err(err) => println!("connection not ready: {err}...\n"),
        }

        if retry == max_try {
            eprintln!("err: unable to connect to manager\n");
            break;
        }

        retry += 1;
    }

    // create a service, create token and stop manager
    let config = ServiceConfig::default();
    let id = PPDrive::add(config, port)?;

    PPDrive::create_client(port, id, "Test Client".to_string())?;
    PPDrive::stop(port)?;

    Ok(())
}
