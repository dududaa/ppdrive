use std::str::FromStr;

use tokio::io::{self, AsyncBufReadExt, BufReader, Stdin};

use crate::errors::AppError;

use super::{AppConfig, CONFIG_FILENAME};

pub async fn run_configor() -> Result<(), AppError> {
    let mut config = AppConfig::build().await?;

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin);

    // port
    println!("What port should we run PPDRIVE on? ENTER to skip");
    println!("current: {}", config.base.port);

    let port = parse_int(&mut reader, config.base.port).await?;
    config.base.port = port;

    // database url
    loop {
        println!("Please specify database URL for PPDRIVE. ENTER to skip");
        println!("current: {}", config.base.database_url);

        let url = parse_str(&mut reader, config.base.database_url()).await?;

        if !url.is_empty() {
            config.base.database_url = url;
            break;
        }
    }

    // allowed origins
    println!(
        "URL(s) to be allowed by PPDRIVE CORS policy, each separated by a comma. ENTER to skip"
    );
    println!("Input * to allow all origins");
    println!("current: {}", config.base.allowed_origins);

    let origins = parse_int(&mut reader, config.base.allowed_origins).await?;
    config.base.allowed_origins = origins;

    // max upload size
    println!("Maximum upload size in MegaBytes (MB). ENTER to skip");
    println!("current: {}", config.file_upload.max_upload_size);

    let muz = parse_int(&mut reader, config.file_upload.max_upload_size).await?;
    config.file_upload.max_upload_size = muz;

    let updated =
        toml::to_string_pretty(&config).map_err(|err| AppError::InitError(err.to_string()))?;

    tokio::fs::write(CONFIG_FILENAME, updated).await?;
    Ok(())
}

async fn parse_int<T: FromStr>(reader: &mut BufReader<Stdin>, default: T) -> Result<T, AppError> {
    let mut input = String::new();
    reader.read_line(&mut input).await?;

    let output = input.trim().parse().unwrap_or(default);
    Ok(output)
}

async fn parse_str(reader: &mut BufReader<Stdin>, default: &str) -> Result<String, AppError> {
    let mut input = String::new();
    reader.read_line(&mut input).await?;

    let mut output = input.trim();
    if output.is_empty() {
        output = default
    }

    Ok(output.to_string())
}
