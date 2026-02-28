use miette::Result;
use shell_escape::escape;

use crate::{
    cli::{GlobalArgs, PrintArgs, PrintCommand},
    state::State,
};

pub async fn print(global: GlobalArgs, args: PrintArgs) -> Result<()> {
    let mut state = State::new(global)?;
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
