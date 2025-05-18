mod prelude;
mod error;
mod lcu;
mod game_client;

use prelude::*;
use game_client::{GameClient, GameSettings};
use tokio::time::{self, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    println!("League Auto Accept - Rust Edition");
    println!("Waiting for League Client to start...");

    loop {
        match GameClient::new().await {
            Ok(mut client) => {
                println!("Connected to League Client!");

                // Configure default settings
                let mut settings = GameSettings::default();
                settings.auto_accept_enabled = true;
                settings.auto_restart_queue = true;
                client.update_settings(settings);

                println!("Auto accept enabled and ready!");
                
                // Run the main loop
                match client.run().await {
                    Ok(_) => println!("Client closed normally"),
                    Err(e) => {
                        println!("Lost connection to League Client: {}", e);
                        println!("Waiting for League Client to restart...");
                    }
                }
            }
            Err(e) => {
                println!("Failed to connect to League Client: {}", e);
                time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
