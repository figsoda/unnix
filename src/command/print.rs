use miette::Result;
use shell_escape::escape;

use crate::{
    cli::{GlobalArgs, PrintArgs, PrintCommand, PrintEnvArgs},
    state::State,
};

pub async fn print(global: GlobalArgs, args: PrintArgs) -> Result<()> {
    match args.command {
        PrintCommand::Env(args) => env(global, args).await,
    }
}

async fn env(global: GlobalArgs, args: PrintEnvArgs) -> Result<()> {
    let state = State::new_locked(global, args.system.try_into()?).await?;
    for (name, value) in state.env().await? {
        println!("export {name}={}", escape(value.into()));
    }
    Ok(())
}
