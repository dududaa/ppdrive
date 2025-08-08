use ppdrive_rest::start_server;

use crate::error::{self, CliResult};
pub enum Command {
    Version,
    Start,
}

impl Command {
    pub async fn run(&self, args: Vec<String>) -> CliResult<()> {
        use Command::*;

        match self {
            Version => {
                let n = std::env::var("CARGO_PKG_NAME")?;
                let v = std::env::var("CARGO_PKG_VERSION")?;

                println!("{n}: {v}");
            }
            Start => {
                start_server().await?;
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
        } else if value == "start" {
            Ok(Start)
        } else {
            Err(error::CliError::CommandError(
                "unrecognized command".to_string(),
            ))
        }
    }
}
