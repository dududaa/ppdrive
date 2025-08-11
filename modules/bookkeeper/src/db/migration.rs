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

pub async fn run_migrations(url: &str) -> DBResult<()> {
    let config = ConfigBuilder::new()
        .db_url(url)
        .migrations_dir(root_dir()?.join("migrations"))
        .build();

    Clients::write_stream(&config).await?;
    Buckets::write_stream(&config).await?;
    Mimes::write_stream(&config).await?;
    BucketMimes::write_stream(&config).await?;
    Users::write_stream(&config).await?;
    Assets::write_stream(&config).await?;
    AssetPermissions::write_stream(&config).await?;

    run_modeller(&config).await?;
    Ok(())
}
