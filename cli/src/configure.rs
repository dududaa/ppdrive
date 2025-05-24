use std::str::FromStr;

use ppdrive_core::config::{AppConfig, ConfigUpdater};

use crate::error::CliResult;
use tokio::io::{AsyncBufReadExt, BufReader, Stdin};

/// run tool with --configure arg
pub async fn run_configor() -> CliResult<()> {
    let mut config = AppConfig::load().await?;
    let mut data = ConfigUpdater::default();
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);

    // port
    let port = config.server().port();
    println!("What port should we run PPDRIVE on? ENTER to skip");
    println!("current: {}", port);

    data.server_port = parse_int(&mut reader, *port).await.ok();

    // database url
    loop {
        let url = config.db().url();
        println!("Please specify database URL for PPDRIVE. ENTER to skip");
        println!("current: {}", url);
        let url = parse_str(&mut reader, url).await?;

        if !url.is_empty() {
            data.db_url = Some(url);
            break;
        }
    }

    // allowed origins
    let origins = config.server().allowed_origins();
    println!(
        "URL(s) to be allowed by PPDRIVE CORS policy, each separated by a comma. ENTER to skip"
    );
    println!("Input * to allow all origins");
    println!("current: {}", origins);

    data.allowed_urls = parse_str(&mut reader, origins).await.ok();

    // max upload size
    let max_upload = config.server().max_upload_size();
    println!("Maximum upload size in MegaBytes (MB). ENTER to skip");
    println!("current: {}", max_upload);

    data.max_upload_size = parse_int(&mut reader, *max_upload).await.ok();

    config.update(data).await?;
    Ok(())
}

async fn parse_int<T: FromStr>(reader: &mut BufReader<Stdin>, default: T) -> CliResult<T> {
    let mut input = String::new();
    reader.read_line(&mut input).await?;

    let output = input.trim().parse().unwrap_or(default);
    Ok(output)
}

async fn parse_str(reader: &mut BufReader<Stdin>, default: &str) -> CliResult<String> {
    let mut input = String::new();
    reader.read_line(&mut input).await?;

    let mut output = input.trim();
    if output.is_empty() {
        output = default
    }

    Ok(output.to_string())
}
