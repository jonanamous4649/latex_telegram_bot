// anyhow provides the Result type and error handling helpers
use anyhow::{Context, Result};

// std = Standard Library (built into Rust)
use std::env;           // For reading environment variables
use std::path::PathBuf; // For file paths

// pub = Public (other files can use this)
// struct = Defines a custom data type
pub struct Config {
    // String = Owned text (like str in Python)
    pub telegram_token: String,

    // i64 = 64-bit signed integer
    // Telegram chat IDs are numbers
    pub chat_id: i64,

    // PathBuf = A file system path (cross-platform)
    pub output_dir: PathBuf,

    // Command to run LaTeX (usually "pdflatex")
    pub latex_cmd: String,
}

impl Config {
    // from_env() creatse a Config from environment variables
    // -> Result<Self> means "returns either Ok(Config) or an error"
    pub fn from_env() -> Result<Self> {

        // Read TELEGRAM_BOT_TOKEN from environment
        // .context() adds a helpful message if this fails
        let telegram_token: = env::var("Telegram")
    }
}