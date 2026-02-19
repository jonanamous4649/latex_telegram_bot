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
        let telegram_token = env::var("TELEGRAM_BOT_TOKEN")
            .context("TELEGRAM_BOT_TOKEN not set! Get it from @BotFather")?;

        // Read Telegram chat ID
        let chat_id = env::var("TELEGRAM_CHAT_ID")
            .context("TELEGRAM_CHAT_ID not set! Check getUpdates API")?
            .parse::<i64>()
            .context("TELEGRAM_CHAT_ID must be a number!")?;

        // Read output directory
        let output_dir = env::var("OUTPUT_DIR")
            // .map() transforms Ok value if exists
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("output"));

        // Read LaTeX command
        let latex_cmd = env::var("LATEX_CMD")
            .unwrap_or_else(|_| "pdflatex".to_string());
        
        // Constructor
        Ok(Config {
            telegram_token,
            chat_id,
            output_dir,
            latex_cmd,
        })
    }
}