pub mod engine;
pub mod memory;
pub mod net;
pub mod resp;

use crate::net::server::{Config, Server};
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:6379".to_string());
    let aof_path = env::var("AOF_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/appendonly.aof"));

    let config = Config::new(addr, aof_path);
    let server = Server::new(config);

    println!("Starting server...");

    server.run().await?;

    Ok(())
}
