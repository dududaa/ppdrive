use command::Command;
use ppdrive_core::{
    config::{AppConfig, get_config_path},
    db::init_db,
    tools::secrets::AppSecrets,
};

mod command;
mod configure;
mod error;

#[tokio::main]
async fn main() -> Result<(), error::CliError> {
    let config_path = get_config_path()?;
    let config = AppConfig::load(config_path).await?;
    let url = config.db().url();

    let db = init_db(url).await?;
    let secrets = AppSecrets::read().await?;

    let mut args: Vec<String> = std::env::args().collect();
    match args.get(1) {
        Some(cmd) => {
            let cmd: Command = cmd.try_into()?;
            let cmd_args = args.split_off(2);
            cmd.run(cmd_args, &db, &secrets).await?;
        }
        None => panic!("please provide a command"),
    }

    Ok(())
}
