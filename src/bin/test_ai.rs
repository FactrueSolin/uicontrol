use anyhow::Result;
use rig::{client::CompletionClient, completion::Prompt};

use uicontrol::ai::{get_openai_client, load_openai_config};

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_openai_config()?;
    let client = get_openai_client()?;

    let agent = client.agent(config.model_name).build();
    let response = agent
        .prompt("Hello, please respond with a short greeting.")
        .await?;

    println!("AI response:\n{}", response);

    Ok(())
}
