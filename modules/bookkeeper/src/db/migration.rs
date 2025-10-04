use std::path::PathBuf;

use modeller::prelude::*;
use ppd_shared::tools::root_dir;

use crate::{
    DBResult,
    models::{
        asset::Assets,
        bucket::Buckets,
        client::Clients,
        mime::{BucketMimes, Mimes},
        permission::AssetPermissions,
        user::Users,
    },
};

fn modeller_path() -> DBResult<PathBuf> {
    let path = root_dir()?.join("modeller");
    Ok(path)
}

pub async fn run_migrations(url: &str) -> DBResult<()> {
    let mut config = ConfigBuilder::new()
        .db_url(url)
        .metadata_path(modeller_path()?)
        .build();

    Clients::write_stream(&mut config);
    Buckets::write_stream(&mut config);
    Mimes::write_stream(&mut config);
    BucketMimes::write_stream(&mut config);
    Users::write_stream(&mut config);
    Assets::write_stream(&mut config);
    AssetPermissions::write_stream(&mut config);

    run_modeller(&config).await?;
    Ok(())
}

pub async fn clean_db() -> DBResult<()> {
    let path = modeller_path()?;
    tokio::fs::remove_file(path)
        .await
        .map_err(|err| Error::InternalError(err.to_string()))?;

    Ok(())
}
