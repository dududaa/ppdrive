use ppdrive_core::{
    RBatis,
    tools::{
        create_client, regenerate_token,
        secrets::{AppSecrets, generate_secret_file},
    },
};

use crate::{
    configure::run_configor,
    error::{self, CliResult},
};
pub enum Command {
    Version,
    CreateClient,
    ClientToken,
    GenerateSecrets,
    Configure,
}

impl Command {
    pub async fn run(&self, args: Vec<String>, db: &RBatis, secrets: &AppSecrets) -> CliResult<()> {
        use Command::*;

        match self {
            Version => {
                let n = std::env::var("CARGO_PKG_NAME")?;
                let v = std::env::var("CARGO_PKG_VERSION")?;

                println!("{n}: {v}");
            }
            CreateClient | ClientToken => {
                let next_arg = args.get(1);
                let next_arg_name = match self {
                    CreateClient => "name",
                    ClientToken => "id",
                    _ => "",
                };

                match next_arg {
                    Some(next) => {
                        let token = match self {
                            CreateClient => create_client(db, secrets, next).await?,
                            ClientToken => regenerate_token(db, secrets, next).await?,
                            _ => "".to_string(),
                        };

                        println!("client token {token}")
                    }
                    None => panic!("client's {next_arg_name} must be provided."),
                }
            }
            GenerateSecrets => {
                generate_secret_file().await?;
                println!("secrets generated successfully!");
            }
            Configure => {
                run_configor().await?;
                println!("configuration saved")
            }
        }

        Ok(())
    }
}

impl<'a> TryFrom<&'a String> for Command {
    type Error = error::CliError;
    fn try_from(value: &'a String) -> CliResult<Self> {
        use Command::*;

        let value = value.as_str();
        if ["--version", "-v"].contains(&value) {
            Ok(Version)
        } else if value == "create_client" {
            Ok(CreateClient)
        } else if value == "token" {
            Ok(ClientToken)
        } else if value == "xgen" {
            Ok(GenerateSecrets)
        } else if value == "configure" {
            Ok(Configure)
        } else {
            Err(error::CliError::CommandError(
                "unrecognized command".to_string(),
            ))
        }
    }
}
