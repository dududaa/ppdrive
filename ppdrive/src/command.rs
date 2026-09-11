use crate::subs::{AssetCommand, BucketCommand, ClientCommand, UserCommand};
use clap::{Parser, Subcommand};
use shared::db::client::{create_client, regenerate_token};
use shared::db::{asset, bucket, user};
use shared::config::AppConfig;
use shared::db::Database;
use shared::secrets::AppSecrets;
use shared::asset_owner_id;
use shared::AssetOwnerName;
use std::process::Command;

/// PPDRIVE is a free, open-source object storage service built with Rust for speed, security,
/// and reliability.
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

/// Parse CLI arguments and execute the corresponding subcommand.
impl Cli {
    pub async fn execute(&self) -> Result<(), anyhow::Error> {
        let mut config = AppConfig::read().await?;
        let pool = Database::new(&config.database_url, config.db_pool_size.unwrap_or(10)).await?;
        config.validate_static_folders(&pool).await?;
        AppSecrets::init().await?;
        let secret = AppSecrets::read().await?;

        match &self.command {
            CliCommand::Client { command } => match command {
                ClientCommand::Create { client_name } => {
                    let client = create_client(&pool, &secret, client_name).await?;

                    println!("Client created successfully!");
                    println!("Client ID: {}", client.id());
                    println!("Client Token: {}", client.token());
                }
                ClientCommand::Refresh { client_id } => {
                    let token = regenerate_token(&pool, &secret, client_id).await?;
                    println!("Client token refreshed successfully!");
                    println!("Client Token: {}", token);
                }
                _ => {}
            },

            CliCommand::Bucket { command } => match command {
                BucketCommand::Create(args) => {
                    let owner_id = shared::db::client::get_id(&args.owner_id, &pool).await?;
                    let data = args.clone().into_data(owner_id);
                    let id = bucket::create(&data, &config.static_folders, &pool).await?;

                    println!("Bucket created successfully!");
                    println!("Bucket ID: {id}");
                }
            },

            CliCommand::Asset { command } => match command {
                AssetCommand::Grant { bucket, path, grantee, grantee_type, permission } => {
                    let bucket_data = bucket::get(&bucket, &pool).await?;
                    let grantee_owner_id = match grantee_type.as_str() {
                        "user" => {
                            let user_id = user::get_id(&grantee, &pool).await?;
                            asset_owner_id(AssetOwnerName::User, user_id, &pool).await?
                        }
                        _ => {
                            let client_id = shared::db::client::get_id(&grantee, &pool).await?;
                            asset_owner_id(AssetOwnerName::Client, client_id, &pool).await?
                        }
                    };

                    let cleaned_path = path.trim_start_matches('/');
                    let asset = asset::get_by_bucket_and_path(&pool, bucket_data.id, cleaned_path).await?
                        .ok_or_else(|| anyhow::anyhow!("file not found. Upload the file first to register it."))?;

                    asset::grant(&pool, asset.id, grantee_owner_id, *permission).await?;
                    println!("Permission '{permission}' granted to {grantee_type} '{grantee}' on '{cleaned_path}'");
                }
                AssetCommand::Revoke { bucket, path, grantee, grantee_type } => {
                    let bucket_data = bucket::get(&bucket, &pool).await?;
                    let grantee_owner_id = match grantee_type.as_str() {
                        "user" => {
                            let user_id = user::get_id(&grantee, &pool).await?;
                            asset_owner_id(AssetOwnerName::User, user_id, &pool).await?
                        }
                        _ => {
                            let client_id = shared::db::client::get_id(&grantee, &pool).await?;
                            asset_owner_id(AssetOwnerName::Client, client_id, &pool).await?
                        }
                    };

                    let cleaned_path = path.trim_start_matches('/');
                    let asset = asset::get_by_bucket_and_path(&pool, bucket_data.id, cleaned_path).await?
                        .ok_or_else(|| anyhow::anyhow!("file not found"))?;

                    asset::revoke(&pool, asset.id, grantee_owner_id).await?;
                    println!("Permission revoked for {grantee_type} '{grantee}' on '{cleaned_path}'");
                }
                AssetCommand::List { bucket, path } => {
                    let bucket_data = bucket::get(&bucket, &pool).await?;

                    if let Some(path) = path {
                        let cleaned_path = path.trim_start_matches('/');
                        let asset = asset::get_by_bucket_and_path(&pool, bucket_data.id, cleaned_path).await?;
                        match asset {
                            Some(asset) => {
                                let permissions = asset::list_permissions(&pool, asset.id).await?;
                                if permissions.is_empty() {
                                    println!("No permissions found for '{cleaned_path}'");
                                } else {
                                    println!("Permissions for '{cleaned_path}':");
                                    for perm in &permissions {
                                        println!("  {} ({}) -> {}", perm.grantee_name, perm.grantee_type, perm.permission);
                                    }
                                }
                            }
                            None => {
                                println!("File '{cleaned_path}' not found");
                            }
                        }
                    } else {
                        println!("Listing all permissions for bucket '{}' is not yet supported via CLI. Use the API instead.", bucket);
                    }
                }
            },

            CliCommand::Serve { port } => {


                if cfg!(debug_assertions) {
                    Command::new("cargo")
                        .args(["run", "-p", "server"])
                        .arg(port.to_string())
                        .status()?;
                } else {
                    Command::new("./server").arg(port.to_string()).status()?;
                }
            }

            CliCommand::Configure => {
                let editor = std::env::var("VISUAL")
                    .or_else(|_| std::env::var("EDITOR"))
                    .unwrap_or_else(|_| "nano".to_string());
                Command::new(&editor).arg("ppd_config.toml").status()?;
            }

            CliCommand::User { command } => match command {
                UserCommand::Create { email, password } => {
                    user::create(email, password, &pool).await?;
                    println!("User created successfully!");
                    println!("Email: {email}");
                }
            },
        }

        Ok(())
    }
}

#[derive(Subcommand, Debug)]
enum CliCommand {
    Serve {
        #[arg(long = "port", default_value = "8000")]
        port: u16,
    },
    Configure,
    /// create a new client
    Client {
        #[command(subcommand)]
        command: ClientCommand,
    },
    Bucket {
        #[command(subcommand)]
        command: BucketCommand,
    },
    /// manage file-level permissions in private buckets
    Asset {
        #[command(subcommand)]
        command: AssetCommand,
    },
    /// manage user accounts
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
}
