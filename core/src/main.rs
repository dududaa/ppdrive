use command::Command;

mod command;
mod error;

#[tokio::main]
async fn main() -> Result<(), error::CliError> {
    let mut args: Vec<String> = std::env::args().collect();
    match args.get(1) {
        Some(cmd) => {
            let cmd: Command = cmd.try_into()?;
            let cmd_args = args.split_off(2);
            cmd.run(cmd_args).await?;
        }
        None => panic!("please provide a command"),
    }

    Ok(())
}
