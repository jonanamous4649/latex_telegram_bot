// Import external libraries (crates)
use anyhow::Result;     // Easy error handling
use chrono::Local;      // Current date/time

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
    println!("📡 Fetching data...");
    let data = fetch_data().await?;

    // Step 3: Prepare template context
    // Covner our data structure into a format Tera templates can use
    println!("📝 Preparing template...");
    let context = tera::Context::from_serialize(&data)?;

    // Step 4: Render LaTeX to PDF
    // This fills in the template and runs pdflatex
    println!("📄 Compiling LaTeX to PDF...");
    let renderer = LatexRenderer::new(&config.output_dir)?;
    let pdf_path = renderer.render("template.tex", context, "report").await?;

    // Step 5: Send PDF via Telegram
    println!("📤 Sending PDF to Telegram...");
    let sender = TelegramSender::new(&config.telegram_token, config.chat_id);
    let caption = format!(
        "📊 Report generated at {}",
        Local::now().format("%Y-%m-%d %H:%M")
    );
    sender.send_pdf(&pdf_path, &caption).await?;

    // Success!
    println!("✅ Complete! Check your Telegram.");

    // Return Ok(()) means "everything worked, no errors"
    Ok(())
}
