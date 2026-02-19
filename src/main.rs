// Import external libraries (crates)
use anyhow::Result;     // Easy error handling
use chrono::Local;      // Current date/time
use std::path::Pathbuf; // For handling file paths

// Import our own modules (other files we'll create)
mod config;             // Settings like API tokens
mod data_fetcher;       // Gets your data
mod latex_renderer;     // Creates PDF from template
mod telegram_sender;    // Sends PDF via Telegram

// Import specific things from our modules
use config::Config;
use data_fetcher::fetch_data;
use latex_renderer::LatexRenderer;
use telegram_sender::TelegramSender;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting LaTeX Telegram Bot...");

    // Load environment variables from .env file
    // The ? means "if this fails, stop and return the error"
    let _ = dotenvy::dotenv();

    // Step 1: Load configuration (API tokens, etc.)
    // Config::from_env() is a function in config.rs
    println!("⚙️ Loading configuration...");
    let config = Config::from_env()?;

    // Step 2: Fetch data from your sources
    // This calls your Python scripts or database
}
