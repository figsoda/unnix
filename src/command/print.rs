use miette::Result;
use shell_escape::escape;

use crate::{
    cli::{PrintArgs, PrintCommand},
    state::State,
};

pub async fn print(mut state: State, args: PrintArgs) -> Result<()> {
    state.lock().await?;
    match args.command {
        PrintCommand::Env => env(state).await,
    }
}

async fn env(state: State) -> Result<()> {
    for (name, value) in state.env().await? {
        println!("export {name}={}", escape(value.into()));
    }
    Ok(())
}
