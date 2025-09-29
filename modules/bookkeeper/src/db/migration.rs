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
    let mut config = ConfigBuilder::new()
        .db_url(url)
        .metadata_path(root_dir()?.join("modeller"))
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
