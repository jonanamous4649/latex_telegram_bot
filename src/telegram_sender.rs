use anyhow::{Context, Result};
use reqwest::{Client, multipart};
use std::path::Path;


// configs and state for sending Telegram messages
pub struct TelegramSender {
    // HTTP client instance
    // Arc<Client> internally, cheaper to clone
    client: Client,

    // bot token from @BotFather
    // String (owned) because we store it for lifetime of struct
    // token: String,

    // Telegram user ID
    // i64 because Telegram chat IDs can be large
    chat_id: i64,

    // base URL for all Telegram API calls
    base_url: String,
}

impl TelegramSender {

    // constructor - creates new TelegramSender
    // '&str' = borrowed string slice
    // convert to String internally to store it
    pub fn new(token: &str, chat_id: i64) -> Self {

        // build base URL by formatting token into string
        // fomrat!() macro creates a String
        let base_url = format!("https://api.telegram.org/bot{}", token);

        TelegramSender {
            client: Client::new(),      // create new HTTP client with default settings
            // token: token.to_string(),   // convert &str to owned String
            chat_id,
            base_url,
        }
    }

    // send PDF files to Telegram
    // 'caption' = message text to show above the file
    pub async fn send_pdf(&self, pdf_path: &Path, caption: &str) -> Result<()> {
        
        println!(" Uploading {} to Telegram...", pdf_path.display());

        // build API endpoint URL
        let url = format!("{}/sendDocument", self.base_url);

        // read PDF file into memory as bytes
        // tokio::fs:read is async
        // returns Result<Vec<u8>> (vector of bytes)
        let file_bytes = tokio::fs::read(pdf_path).await
            .with_context(|| {
                format!("Failed to read PDF file: {}", pdf_path.display())
            })?;

        // extract filename from path for upload
        // file_name() returns Option<&OsStr>
        // and_then converts Option to Option
        // unwrap or provides default if any step returns None
        let filename = pdf_path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("report.pdf");

        // build multipart from data
        // multipart = HTTP format for sending files with metadata
        let form = multipart::Form::new()
            // text field: chat_id (where to send)
            .text("chat_id", self.chat_id.to_string())

            // text field: caption (message shown with file)
            .text("caption", caption.to_string())

            // text field: parse_mode (formatting style)
            // 'markdown' allows *bold* and _italic_ in caption
            .text("parse_mode", "Markdown".to_string())

            //file field: the actual PDF
            .part("document",
                multipart::Part::bytes(file_bytes)  // file contents as bytes
                .file_name(filename.to_string())        //file name shown in TG
                .mime_str("applicaiton/pdf")?       // MIME type
            );

        // send HTTP POST request
        println!(" Sending request to Telegram API...");
        let response = self.client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .context("Failed to send request to Telegram API")?;

        // check HTTP status code
        let status = response.status();
        let body = response.text().await
            .context("Failed to read response body")?;

        if !status.is_success() {
            anyhow::bail!(
                "Telegram API returned error (HTTP {}): {}",
                status,
                body
            );
        }

        // parse JSON to check for Telegram-level errors
        // serde_json::from_str parses JSON string into Rust type
        // 'enum' value can represent any JSON
        let json: serde_json::Value = serde_json::from_str(&body)
            .context("Invalid JSON response from Telegram")?;

        // json.get("ok") returns Option<&Value>
        // and_then(|v| v.as_bool()) tries to get it as bool
        // unwrap_or(false) sets default to false if the try didn't work
        let ok = json.get("ok")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);



        Ok(()) 
    }
}