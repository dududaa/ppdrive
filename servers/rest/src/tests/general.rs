use ppd_bk::db::migration::run_migrations;
use crate::ServerResult;
use ppd_shared::{config::AppConfig, tools::root_dir};
use serial_test::serial;

#[serial]
#[tokio::test]
async fn test_migration() -> ServerResult<()> {
    let dir = root_dir()?.join("migrations");

    if !dir.is_dir() {
        let config = AppConfig::load().await?;
        let run = run_migrations(config.db().url()).await;

        if let Err(err) = &run {
            println!("err {err}")
        }

        assert!(run.is_ok())
    }

    Ok(())
}